// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use hyper_component::connection::{ConnectionError, Rejection, RejectionKind, Reply};
use hyper_os::{Error, Status};

#[test]
fn provider_death_is_distinct_from_authority_denial_and_backoff()
-> Result<(), Box<dyn std::error::Error>> {
    for (kind, status) in [
        (RejectionKind::NotOffered, Status::ACCESS_DENIED),
        (RejectionKind::Unavailable, Status::PEER_CLOSED),
        (RejectionKind::Restarting, Status::BUSY),
        (RejectionKind::TimedOut, Status::TIMED_OUT),
        (RejectionKind::ResourceLimit, Status::RESOURCE_LIMIT),
    ] {
        let wire = serde_json::to_vec(&Reply {
            error: Some(Rejection::new(kind, "provider generation ended")),
        })?;
        let reply: Reply = serde_json::from_slice(&wire)?;
        let rejection = reply.error.ok_or("rejection disappeared")?;
        assert_eq!(rejection.kind, kind);
        let error = Error::from(ConnectionError::Rejected(rejection));
        assert!(matches!(error, Error::Status(actual) if actual == status));
    }
    Ok(())
}

#[test]
fn transport_failure_does_not_claim_a_server_rejection() {
    let error = ConnectionError::from(Error::Status(Status::PEER_CLOSED));
    assert!(matches!(
        error,
        ConnectionError::Transport(Error::Status(Status::PEER_CLOSED))
    ));
    assert!(matches!(
        Error::from(error),
        Error::Status(Status::PEER_CLOSED)
    ));
}
