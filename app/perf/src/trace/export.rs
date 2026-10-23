// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0
//! Chrome Trace Event JSON accepted by Perfetto. Every derived span requires
//! observed matching boundaries. Gaps invalidate pending spans; an incomplete
//! stream is shown as instants instead of inventing timing across unknown loss.
use super::model::{Capture, Event};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::{self, Write},
};

pub fn jsonl(capture: &Capture, mut output: impl Write) -> io::Result<()> {
    writeln!(output, "{}", metadata(capture))?;
    for event in &capture.events {
        let site = &capture.sites[event.site];
        writeln!(
            output,
            "{}",
            json!({"cpu":event.cpu,"sequence":event.sequence,"time_ns":event.time,
            "category":site.category,"event":site.name,"source":site.source,"line":site.line,"args":fields(capture,event)})
        )?;
    }
    writeln!(
        output,
        "{}",
        json!({"end":true,"events":capture.events.len(),"loss":capture.loss(),"limited":capture.limited})
    )
}
fn metadata(c: &Capture) -> Value {
    let revision = String::from_utf8_lossy(&c.info.revision)
        .trim_end_matches('\0')
        .to_string();
    let losses: Vec<_> = c
        .losses
        .iter()
        .map(|l| json!({"cpu":l.cpu,"overwritten":l.overwritten,"incomplete":l.incomplete}))
        .collect();
    let names: BTreeMap<String, &String> = c
        .names
        .iter()
        .map(|(id, name)| (format!("{id:#x}"), name))
        .collect();
    json!({"format":"hyper-trace-v1","kernel":revision,"generation":c.info.generation,"clock":"monotonic-ns",
        "started_ns":c.info.started_ns,"deadline_ns":c.info.deadline_ns,"categories":c.info.categories,
        "capacity_per_cpu":c.info.capacity,"cpu_count":c.info.cpu_count,"cpu_mask":c.info.cpu_mask, "losses":losses,
        "thread_names":names,"limited":c.limited,"cross_cpu_spans":c.cross_cpu_spans()})
}
fn fields(c: &Capture, event: &Event) -> Value {
    let mut args = serde_json::Map::new();
    for (name, value) in c.sites[event.site].fields.iter().zip(event.values) {
        // Preserve all bits of IDs in JavaScript readers, even above 2^53.
        args.insert(
            name.clone(),
            if matches!(
                name.as_str(),
                "thread"
                    | "prev"
                    | "next"
                    | "waker"
                    | "process"
                    | "channel"
                    | "vm"
                    | "domain"
                    | "tag"
            ) {
                json!(format!("{value:#x}"))
            } else if name == "status" {
                json!(value as i64)
            } else {
                json!(value)
            },
        );
    }
    args.insert("sequence".into(), json!(event.sequence));
    args.insert("cpu".into(), json!(event.cpu));
    args.insert("source".into(), json!(c.sites[event.site].source));
    args.insert("line".into(), json!(c.sites[event.site].line));
    Value::Object(args)
}
struct JsonTrace<W> {
    output: W,
    first: bool,
    origin: u64,
}
impl<W: Write> JsonTrace<W> {
    fn event(&mut self, value: Value) -> io::Result<()> {
        if !self.first {
            self.output.write_all(b",\n")?;
        }
        self.first = false;
        serde_json::to_writer(&mut self.output, &value).map_err(io::Error::other)
    }
    fn name(&mut self, pid: u32, tid: u32, name: &str) -> io::Result<()> {
        self.event(json!({"ph":"M","pid":pid,"tid":tid,"name":if tid==0 {"process_name"} else {"thread_name"},"args":{"name":name}}))
    }
    fn time(&self, ns: u64) -> f64 {
        ns.saturating_sub(self.origin) as f64 / 1000.0
    }
    fn span(
        &mut self,
        track: (u32, u32),
        name: &str,
        category: &str,
        time: std::ops::Range<u64>,
        args: Value,
    ) -> io::Result<()> {
        let (pid, tid) = track;
        let (begin, end) = (time.start, time.end);
        if end < begin {
            return Ok(());
        }
        self.event(json!({"ph":"X","pid":pid,"tid":tid,"name":name,"cat":category,"ts":self.time(begin),"dur":(end-begin) as f64 / 1000.0,"args":args}))
    }
}
/// CPU lanes represent observed on-CPU intervals. Per-thread lanes show elapsed
/// syscall/VM/fault time (which can include descheduling), and runnable latency.
/// These are native tracks, not fabricated Linux `sched_switch` records.
pub fn perfetto(c: &Capture, output: impl Write) -> io::Result<()> {
    let mut w = JsonTrace {
        output,
        first: true,
        origin: c.info.started_ns,
    };
    w.output.write_all(b"{\"traceEvents\":[\n")?;
    w.name(1, 0, "HypeR CPU execution")?;
    w.name(2, 0, "HypeR threads (elapsed time)")?;
    w.name(3, 0, "HypeR IRQ service")?;
    w.name(4, 0, "HypeR block requests")?;
    for cpu in 0..c.info.cpu_count as u32 {
        w.name(1, cpu + 1, &format!("CPU {cpu}"))?;
        w.name(3, cpu + 1, &format!("CPU {cpu} IRQ"))?;
    }
    // Export-local small IDs avoid truncating the kernel's 64-bit identifiers.
    let mut threads = BTreeMap::new();
    for event in &c.events {
        for key in ["thread", "prev", "next", "waker"] {
            if let Some(id) = c.value(event, key)
                && !threads.contains_key(&id)
            {
                let tid = threads.len() as u32 + 1;
                threads.insert(id, tid);
                w.name(
                    2,
                    tid,
                    &c.names
                        .get(&id)
                        .cloned()
                        .unwrap_or_else(|| format!("thread {id:#x}")),
                )?;
            }
        }
    }
    let mut previous_sequence = vec![None; c.info.cpu_count as usize];
    let mut running: Vec<Option<(&Event, u64)>> = vec![None; c.info.cpu_count as usize];
    let mut pending: BTreeMap<(&str, u64), &Event> = BTreeMap::new();
    let mut ready: BTreeMap<u64, &Event> = BTreeMap::new();
    let mut irq: BTreeMap<usize, Vec<&Event>> = BTreeMap::new();
    let mut io: BTreeMap<(u64, u64), &Event> = BTreeMap::new();
    let mut io_lanes = BTreeMap::new();
    let mut depth = BTreeMap::<u64, u64>::new();
    let uncertain = !c.cross_cpu_spans();
    for event in &c.events {
        let site = &c.sites[event.site];
        let gap = previous_sequence[event.cpu]
            .is_some_and(|n: u64| n.checked_add(1) != Some(event.sequence));
        previous_sequence[event.cpu] = Some(event.sequence);
        if gap {
            running[event.cpu] = None;
            // A migrated Thread or request completion may live on another CPU.
            pending.clear();
            ready.clear();
            irq.clear();
            io.clear();
            depth.clear();
        }
        let thread = c.value(event, "thread");
        let tid = thread.and_then(|id| threads.get(&id).copied());
        // Preserve raw observations, including every scalar and exact source.
        w.event(json!({"ph":"i","s":"t","pid":if tid.is_some(){2}else{1},"tid":tid.unwrap_or(event.cpu as u32+1),
            "name":site.name,"cat":site.category,"ts":w.time(event.time),"args":fields(c,event)}))?;
        if gap {
            w.event(json!({"ph":"i","s":"t","pid":1,"tid":event.cpu+1,"name":"TRACE GAP","ts":w.time(event.time),"args":{"sequence":event.sequence}}))?;
        }
        if site.name == "sched_switch" {
            if let (Some(prev), Some(next)) = (c.value(event, "prev"), c.value(event, "next")) {
                if !c.losses[event.cpu].incomplete
                    && let Some((begin, id)) = running[event.cpu]
                    && id == prev
                {
                    let name = c
                        .names
                        .get(&id)
                        .cloned()
                        .unwrap_or_else(|| format!("thread {id:#x}"));
                    w.span((1,event.cpu as u32+1),&name,"scheduler",begin.time..event.time,
                        json!({"thread":format!("{id:#x}"),"class":c.value(begin,"next_class"),"end_state":c.value(event,"prev_state")}))?;
                }
                running[event.cpu] = Some((event, next));
                if !uncertain
                    && let Some(begin) = ready.remove(&next)
                    && let Some(tid) = threads.get(&next)
                {
                    w.span(
                        (2, *tid),
                        "Runnable",
                        "scheduler",
                        begin.time..event.time,
                        fields(c, begin),
                    )?;
                }
                if c.value(event, "prev_state") == Some(1) {
                    ready.entry(prev).or_insert(event);
                }
            }
        } else if site.name == "sched_wakeup"
            && let Some(thread) = thread
        {
            ready.entry(thread).or_insert(event);
        }
        if !uncertain && let Some(thread) = thread {
            let group = match site.name.as_str() {
                "syscall_enter" | "syscall_exit" => Some(("syscall", site.name == "syscall_enter")),
                "vcpu_enter" | "vcpu_exit" => Some(("vCPU run", site.name == "vcpu_enter")),
                "page_fault_begin" | "page_fault_end" => {
                    Some(("page fault", site.name == "page_fault_begin"))
                }
                _ => None,
            };
            if let Some((name, begin)) = group {
                if begin {
                    pending.insert((name, thread), event);
                } else if let Some(start) = pending.remove(&(name, thread))
                    && let Some(tid) = tid
                {
                    let mut args = fields(c, start);
                    args["completion"] = fields(c, event);
                    w.span((2, tid), name, &site.category, start.time..event.time, args)?;
                }
            }
        }
        if !c.losses[event.cpu].incomplete && site.name == "irq_enter" {
            irq.entry(event.cpu).or_default().push(event);
        }
        if !c.losses[event.cpu].incomplete
            && site.name == "irq_exit"
            && let Some(start) = irq.get_mut(&event.cpu).and_then(Vec::pop)
            && c.value(start, "irq") == c.value(event, "irq")
        {
            w.span(
                (3, event.cpu as u32 + 1),
                &format!("IRQ {}", c.value(event, "irq").unwrap_or(0)),
                "irq",
                start.time..event.time,
                fields(c, start),
            )?;
        }
        if !uncertain
            && let (Some(domain), Some(tag)) = (c.value(event, "domain"), c.value(event, "tag"))
        {
            let lane = if let Some(id) = io_lanes.get(&domain) {
                *id
            } else {
                let id = io_lanes.len() as u32 + 1;
                io_lanes.insert(domain, id);
                w.name(4, id, &format!("block device {domain:#x}"))?;
                id
            };
            if site.name == "block_submit" {
                io.insert((domain, tag), event);
                *depth.entry(domain).or_default() += 1;
            } else if let Some(start) = io.remove(&(domain, tag)) {
                if let Some(depth) = depth.get_mut(&domain) {
                    *depth = depth.saturating_sub(1);
                }
                let name = if site.name == "block_abort" {
                    "aborted"
                } else if c.value(start, "read") == Some(1) {
                    "read"
                } else {
                    "write/control"
                };
                let mut args = fields(c, start);
                args["completion"] = fields(c, event);
                w.span((4, lane), name, "io", start.time..event.time, args)?;
            }
            w.event(json!({"ph":"C","pid":4,"tid":lane,"name":format!("Outstanding device {domain:#x}"),"ts":w.time(event.time),"args":{"observed requests":depth.get(&domain).copied().unwrap_or(0)}}))?;
        }
    }
    // Deliberately do not close a running slice at the capture deadline: a final
    // switch/completion may never have been observed.
    for loss in &c.losses {
        if loss.overwritten != 0 || loss.incomplete {
            w.event(
                json!({"ph":"i","s":"t","pid":1,"tid":loss.cpu+1,"name":"TRACE LOSS","ts":0,
                "args":{"overwritten":loss.overwritten,"incomplete":loss.incomplete}}),
            )?;
        }
    }
    w.output
        .write_all(b"\n],\"displayTimeUnit\":\"ns\",\"metadata\":")?;
    serde_json::to_writer(&mut w.output, &metadata(c)).map_err(io::Error::other)?;
    w.output.write_all(b"}\n")
}
