use hyper::cpu::{CpuIndex, MAX_CPUS, PerCpu};

#[test]
fn logical_cpu_index_rejects_out_of_capacity_values() {
    assert_eq!(CpuIndex::new(0), Some(CpuIndex::BOOT));
    assert_eq!(
        CpuIndex::new(MAX_CPUS - 1).map(CpuIndex::get),
        Some(MAX_CPUS - 1)
    );
    assert_eq!(CpuIndex::new(MAX_CPUS), None);
    assert_eq!(CpuIndex::new(usize::MAX), None);
}

#[test]
fn per_cpu_storage_requires_a_validated_index() {
    let mut values = PerCpu::new([0usize; MAX_CPUS]);
    let last = super::require_some(CpuIndex::new(MAX_CPUS - 1));

    values[CpuIndex::BOOT] = 7;
    values[last] = 11;

    assert_eq!(values[CpuIndex::BOOT], 7);
    assert_eq!(values[last], 11);
    assert_eq!(values.iter().count(), MAX_CPUS);
}
