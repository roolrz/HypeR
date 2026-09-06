// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::model::{
    CapabilityBinding, CapabilityOperation, MAX_BINDING_NAME_BYTES, MAX_CAPABILITIES_PER_SERVICE,
    MAX_DEPENDENCY_EDGES, MAX_IMAGE_PATH_BYTES, MAX_PURPOSE_NAME_BYTES, MAX_RIGHT_NAME_BYTES,
    MAX_SERVICE_NAME_BYTES, MAX_SERVICES, Manifest, RestartPolicy,
};

/// Declarative ceiling for one authority which init may resolve at launch.
///
/// This is policy metadata, not an object reference. The runtime adapter must
/// resolve and revalidate the real handle immediately before `ProcessBuilder`
/// commit; a validated plan can never manufacture authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthorityDeclaration<'policy> {
    pub provider: Option<&'policy str>,
    pub object_kind: u32,
    pub rights: u64,
    /// The named source may be consumed and transferred exactly once.
    pub movable: bool,
    /// The named source may be retained while a duplicate is transferred.
    pub duplicable: bool,
    /// The named source is a typed factory which can create fresh authority.
    pub creatable: bool,
}

/// One service-contract name resolved to a typed startup-stack purpose.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StartupPurposeDeclaration {
    pub value: u32,
    pub object_kind: u32,
}

/// Adapter from manifest vocabulary to the actual bootstrap authority policy.
pub trait AuthorityPolicy {
    fn authority<'policy>(&'policy self, source: &str) -> Option<AuthorityDeclaration<'policy>>;

    /// Resolves a symbolic purpose in the contract selected by `image`.
    fn startup_purpose(&self, image: &str, name: &str) -> Option<StartupPurposeDeclaration>;

    /// Resolves one named right to exactly one bit.
    fn right(&self, name: &str) -> Option<u64>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidationErrorKind {
    EmptyManifest,
    InvalidServiceName,
    InvalidImagePath,
    DuplicateServiceName,
    InvalidDependencyName,
    DuplicateDependency,
    SelfDependency,
    UnknownDependency,
    TooManyDependencyEdges,
    DependencyCycle,
    InvalidBindingName,
    DuplicateCapabilityPurpose,
    InvalidPurposeName,
    InvalidPurposeDeclaration,
    UnknownCapabilityPurpose,
    UnknownAuthority,
    UnknownAuthorityProvider,
    MissingProviderDependency,
    ObjectKindMismatch,
    InvalidRightName,
    UnknownRight,
    InvalidRightDeclaration,
    DuplicateRight,
    RightsEscalation,
    TransferForbidden,
    DuplicateForbidden,
    CreateForbidden,
    RestartConsumesAuthority,
    MoveSourceReused,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidationError {
    kind: ValidationErrorKind,
    service: Option<usize>,
    capability: Option<usize>,
}

impl ValidationError {
    pub const fn kind(self) -> ValidationErrorKind {
        self.kind
    }

    pub const fn service(self) -> Option<usize> {
        self.service
    }

    pub const fn capability(self) -> Option<usize> {
        self.capability
    }
}

/// Deterministic topological order plus resolved, attenuated capability facts.
#[derive(Debug, Eq, PartialEq)]
pub struct LaunchPlan {
    service_count: usize,
    order: [usize; MAX_SERVICES],
    rights: [[u64; MAX_CAPABILITIES_PER_SERVICE]; MAX_SERVICES],
    kinds: [[u32; MAX_CAPABILITIES_PER_SERVICE]; MAX_SERVICES],
    purposes: [[u32; MAX_CAPABILITIES_PER_SERVICE]; MAX_SERVICES],
}

impl LaunchPlan {
    pub const fn service_count(&self) -> usize {
        self.service_count
    }

    pub fn service_index(&self, launch_position: usize) -> Option<usize> {
        self.order
            .get(launch_position)
            .copied()
            .filter(|_| launch_position < self.service_count)
    }

    pub fn capability_rights(&self, service: usize, capability: usize) -> Option<u64> {
        if service >= self.service_count || capability >= MAX_CAPABILITIES_PER_SERVICE {
            return None;
        }
        Some(self.rights[service][capability])
    }

    pub fn capability_kind(&self, service: usize, capability: usize) -> Option<u32> {
        if service >= self.service_count || capability >= MAX_CAPABILITIES_PER_SERVICE {
            return None;
        }
        Some(self.kinds[service][capability])
    }

    pub fn capability_purpose(&self, service: usize, capability: usize) -> Option<u32> {
        if service >= self.service_count || capability >= MAX_CAPABILITIES_PER_SERVICE {
            return None;
        }
        Some(self.purposes[service][capability])
    }
}

/// Validates the complete graph before any `ProcessBuilder` may start a service.
pub fn validate(
    manifest: &Manifest<'_>,
    policy: &impl AuthorityPolicy,
) -> Result<LaunchPlan, ValidationError> {
    if manifest.services.is_empty() {
        return Err(error(ValidationErrorKind::EmptyManifest, None, None));
    }
    validate_service_identities(manifest)?;
    validate_dependencies(manifest)?;

    let mut plan = LaunchPlan {
        service_count: manifest.services.len(),
        order: [0; MAX_SERVICES],
        rights: [[0; MAX_CAPABILITIES_PER_SERVICE]; MAX_SERVICES],
        kinds: [[0; MAX_CAPABILITIES_PER_SERVICE]; MAX_SERVICES],
        purposes: [[0; MAX_CAPABILITIES_PER_SERVICE]; MAX_SERVICES],
    };
    validate_capabilities(manifest, policy, &mut plan)?;
    build_topological_order(manifest, &mut plan)?;
    Ok(plan)
}

fn validate_service_identities(manifest: &Manifest<'_>) -> Result<(), ValidationError> {
    for (index, service) in manifest.services.iter().enumerate() {
        if !valid_identifier(service.name, MAX_SERVICE_NAME_BYTES) {
            return Err(error(
                ValidationErrorKind::InvalidServiceName,
                Some(index),
                None,
            ));
        }
        if !valid_image_path(service.image, MAX_IMAGE_PATH_BYTES) {
            return Err(error(
                ValidationErrorKind::InvalidImagePath,
                Some(index),
                None,
            ));
        }
        for previous in 0..index {
            if manifest
                .services
                .get(previous)
                .map(|candidate| candidate.name)
                == Some(service.name)
            {
                return Err(error(
                    ValidationErrorKind::DuplicateServiceName,
                    Some(index),
                    None,
                ));
            }
        }
    }
    Ok(())
}

fn validate_dependencies(manifest: &Manifest<'_>) -> Result<(), ValidationError> {
    let mut edge_count = 0_usize;
    for (service_index, service) in manifest.services.iter().enumerate() {
        for (dependency_index, dependency) in service.dependencies.iter().enumerate() {
            if !valid_identifier(dependency, MAX_SERVICE_NAME_BYTES) {
                return Err(error(
                    ValidationErrorKind::InvalidDependencyName,
                    Some(service_index),
                    None,
                ));
            }
            if *dependency == service.name {
                return Err(error(
                    ValidationErrorKind::SelfDependency,
                    Some(service_index),
                    None,
                ));
            }
            if service
                .dependencies
                .iter()
                .take(dependency_index)
                .any(|candidate| candidate == dependency)
            {
                return Err(error(
                    ValidationErrorKind::DuplicateDependency,
                    Some(service_index),
                    None,
                ));
            }
            if find_service(manifest, dependency).is_none() {
                return Err(error(
                    ValidationErrorKind::UnknownDependency,
                    Some(service_index),
                    None,
                ));
            }
            edge_count = edge_count.checked_add(1).ok_or_else(|| {
                error(
                    ValidationErrorKind::TooManyDependencyEdges,
                    Some(service_index),
                    None,
                )
            })?;
            if edge_count > MAX_DEPENDENCY_EDGES {
                return Err(error(
                    ValidationErrorKind::TooManyDependencyEdges,
                    Some(service_index),
                    None,
                ));
            }
        }
    }
    Ok(())
}

fn validate_capabilities(
    manifest: &Manifest<'_>,
    policy: &impl AuthorityPolicy,
    plan: &mut LaunchPlan,
) -> Result<(), ValidationError> {
    for (service_index, service) in manifest.services.iter().enumerate() {
        for (capability_index, capability) in service.capabilities.iter().enumerate() {
            validate_binding_names(capability, service_index, capability_index)?;
            reject_reused_move_source(manifest, service_index, capability_index, capability)?;
            let declaration = policy.authority(capability.source).ok_or_else(|| {
                error(
                    ValidationErrorKind::UnknownAuthority,
                    Some(service_index),
                    Some(capability_index),
                )
            })?;
            validate_provider_dependency(
                manifest,
                service_index,
                capability_index,
                declaration.provider,
            )?;
            validate_operation(
                capability,
                declaration,
                service.restart,
                service_index,
                capability_index,
            )?;
            let purpose = policy
                .startup_purpose(service.image, capability.purpose)
                .ok_or_else(|| {
                    error(
                        ValidationErrorKind::UnknownCapabilityPurpose,
                        Some(service_index),
                        Some(capability_index),
                    )
                })?;
            if purpose.value == 0 || purpose.object_kind == 0 {
                return Err(error(
                    ValidationErrorKind::InvalidPurposeDeclaration,
                    Some(service_index),
                    Some(capability_index),
                ));
            }
            if plan.purposes[service_index][..capability_index].contains(&purpose.value) {
                return Err(error(
                    ValidationErrorKind::DuplicateCapabilityPurpose,
                    Some(service_index),
                    Some(capability_index),
                ));
            }
            if purpose.object_kind != declaration.object_kind {
                return Err(error(
                    ValidationErrorKind::ObjectKindMismatch,
                    Some(service_index),
                    Some(capability_index),
                ));
            }
            let requested_rights =
                resolve_rights(policy, capability, service_index, capability_index)?;
            if requested_rights & !declaration.rights != 0 {
                return Err(error(
                    ValidationErrorKind::RightsEscalation,
                    Some(service_index),
                    Some(capability_index),
                ));
            }
            plan.rights[service_index][capability_index] = requested_rights;
            plan.kinds[service_index][capability_index] = purpose.object_kind;
            plan.purposes[service_index][capability_index] = purpose.value;
        }
    }
    Ok(())
}

fn validate_binding_names(
    capability: &CapabilityBinding<'_>,
    service: usize,
    index: usize,
) -> Result<(), ValidationError> {
    if !valid_identifier(capability.source, MAX_BINDING_NAME_BYTES) {
        return Err(error(
            ValidationErrorKind::InvalidBindingName,
            Some(service),
            Some(index),
        ));
    }
    if !valid_identifier(capability.purpose, MAX_PURPOSE_NAME_BYTES) {
        return Err(error(
            ValidationErrorKind::InvalidPurposeName,
            Some(service),
            Some(index),
        ));
    }
    Ok(())
}

fn validate_provider_dependency(
    manifest: &Manifest<'_>,
    service_index: usize,
    capability_index: usize,
    provider: Option<&str>,
) -> Result<(), ValidationError> {
    let Some(provider) = provider else {
        return Ok(());
    };
    let Some(provider_index) = find_service(manifest, provider) else {
        return Err(error(
            ValidationErrorKind::UnknownAuthorityProvider,
            Some(service_index),
            Some(capability_index),
        ));
    };
    if provider_index == service_index
        || !manifest.services.get(service_index).is_some_and(|service| {
            service
                .dependencies
                .iter()
                .any(|dependency| *dependency == provider)
        })
    {
        return Err(error(
            ValidationErrorKind::MissingProviderDependency,
            Some(service_index),
            Some(capability_index),
        ));
    }
    Ok(())
}

fn validate_operation(
    capability: &CapabilityBinding<'_>,
    declaration: AuthorityDeclaration<'_>,
    restart: RestartPolicy,
    service: usize,
    index: usize,
) -> Result<(), ValidationError> {
    match capability.operation {
        CapabilityOperation::Move if !declaration.movable => {
            return Err(error(
                ValidationErrorKind::TransferForbidden,
                Some(service),
                Some(index),
            ));
        }
        CapabilityOperation::Move if restart != RestartPolicy::Never => {
            return Err(error(
                ValidationErrorKind::RestartConsumesAuthority,
                Some(service),
                Some(index),
            ));
        }
        CapabilityOperation::Duplicate if !declaration.duplicable => {
            return Err(error(
                ValidationErrorKind::DuplicateForbidden,
                Some(service),
                Some(index),
            ));
        }
        CapabilityOperation::Create if !declaration.creatable => {
            return Err(error(
                ValidationErrorKind::CreateForbidden,
                Some(service),
                Some(index),
            ));
        }
        CapabilityOperation::Move
        | CapabilityOperation::Duplicate
        | CapabilityOperation::Create => {}
    }
    Ok(())
}

fn resolve_rights(
    policy: &impl AuthorityPolicy,
    capability: &CapabilityBinding<'_>,
    service: usize,
    index: usize,
) -> Result<u64, ValidationError> {
    let mut requested = 0_u64;
    for name in capability.rights.iter() {
        if !valid_identifier(name, MAX_RIGHT_NAME_BYTES) {
            return Err(error(
                ValidationErrorKind::InvalidRightName,
                Some(service),
                Some(index),
            ));
        }
        let bit = policy.right(name).ok_or_else(|| {
            error(
                ValidationErrorKind::UnknownRight,
                Some(service),
                Some(index),
            )
        })?;
        if bit == 0 || !bit.is_power_of_two() {
            return Err(error(
                ValidationErrorKind::InvalidRightDeclaration,
                Some(service),
                Some(index),
            ));
        }
        if requested & bit != 0 {
            return Err(error(
                ValidationErrorKind::DuplicateRight,
                Some(service),
                Some(index),
            ));
        }
        requested |= bit;
    }
    Ok(requested)
}

fn reject_reused_move_source(
    manifest: &Manifest<'_>,
    service_index: usize,
    capability_index: usize,
    current: &CapabilityBinding<'_>,
) -> Result<(), ValidationError> {
    for earlier_service_index in 0..=service_index {
        let Some(service) = manifest.services.get(earlier_service_index) else {
            continue;
        };
        let limit = if earlier_service_index == service_index {
            capability_index
        } else {
            service.capabilities.len()
        };
        for earlier_index in 0..limit {
            let Some(earlier) = service.capabilities.get(earlier_index) else {
                continue;
            };
            if earlier.source == current.source
                && (earlier.operation == CapabilityOperation::Move
                    || current.operation == CapabilityOperation::Move)
            {
                return Err(error(
                    ValidationErrorKind::MoveSourceReused,
                    Some(service_index),
                    Some(capability_index),
                ));
            }
        }
    }
    Ok(())
}

fn build_topological_order(
    manifest: &Manifest<'_>,
    plan: &mut LaunchPlan,
) -> Result<(), ValidationError> {
    let mut emitted = [false; MAX_SERVICES];
    for position in 0..manifest.services.len() {
        let next = (0..manifest.services.len()).find(|candidate| {
            !emitted[*candidate]
                && manifest.services.get(*candidate).is_some_and(|service| {
                    service.dependencies.iter().all(|dependency| {
                        find_service(manifest, dependency).is_some_and(|index| emitted[index])
                    })
                })
        });
        let Some(next) = next else {
            return Err(error(ValidationErrorKind::DependencyCycle, None, None));
        };
        emitted[next] = true;
        plan.order[position] = next;
    }
    Ok(())
}

fn find_service(manifest: &Manifest<'_>, name: &str) -> Option<usize> {
    manifest
        .services
        .iter()
        .position(|service| service.name == name)
}

fn valid_identifier(value: &str, maximum_length: usize) -> bool {
    if value.is_empty() || value.len() > maximum_length {
        return false;
    }
    value.bytes().enumerate().all(|(index, byte)| match byte {
        b'a'..=b'z' => true,
        b'0'..=b'9' | b'_' | b'-' if index != 0 => true,
        b'.' if index != 0 && index + 1 != value.len() => true,
        _ => false,
    })
}

fn valid_image_path(path: &str, maximum_length: usize) -> bool {
    if path.len() < 2
        || path.len() > maximum_length
        || !path.starts_with('/')
        || path.ends_with('/')
    {
        return false;
    }
    path.get(1..).is_some_and(|relative| {
        relative.split('/').all(|component| {
            !component.is_empty()
                && component != "."
                && component != ".."
                && component.bytes().all(|byte| {
                    matches!(byte, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'.' | b'_' | b'-')
                })
        })
    })
}

const fn error(
    kind: ValidationErrorKind,
    service: Option<usize>,
    capability: Option<usize>,
) -> ValidationError {
    ValidationError {
        kind,
        service,
        capability,
    }
}
