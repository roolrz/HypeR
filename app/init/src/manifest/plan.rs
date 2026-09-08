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
pub struct AuthorityKey(u16);

impl AuthorityKey {
    /// Creates one policy-local authority identity.
    ///
    /// Keys are opaque to the manifest and need only be unique within one
    /// `AuthorityPolicy` implementation. The runtime consumes the resolved key
    /// instead of interpreting the manifest's source string a second time.
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    pub const fn as_raw(self) -> u16 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthorityDeclaration<'policy> {
    pub key: AuthorityKey,
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
///
/// The required and allowed masks define the complete authority contract for
/// this purpose. The manifest may attenuate within that interval, but cannot
/// silently grant an implementation more authority merely because the source
/// object happens to support it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StartupPurposeDeclaration {
    pub value: u32,
    pub object_kind: u32,
    pub required_rights: u64,
    pub allowed_rights: u64,
}

/// One capability grant fully resolved by manifest validation.
///
/// Keeping these facts together prevents launch code from accidentally
/// combining the authority, operation, kind, purpose, or rights of different
/// capability rows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityGrant {
    authority: AuthorityKey,
    operation: CapabilityOperation,
    object_kind: u32,
    purpose: u32,
    rights: u64,
}

impl CapabilityGrant {
    pub const fn authority(self) -> AuthorityKey {
        self.authority
    }

    pub const fn operation(self) -> CapabilityOperation {
        self.operation
    }

    pub const fn object_kind(self) -> u32 {
        self.object_kind
    }

    pub const fn purpose(self) -> u32 {
        self.purpose
    }

    pub const fn rights(self) -> u64 {
        self.rights
    }
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
    ConflictingAuthorityKey,
    InvalidInitialVmImage,
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
    MissingRequiredRights,
    ExcessPurposeRights,
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
pub struct LaunchPlan<'manifest> {
    service_count: usize,
    initial_vm_image: Option<&'manifest str>,
    order: [usize; MAX_SERVICES],
    grants: [[Option<CapabilityGrant>; MAX_CAPABILITIES_PER_SERVICE]; MAX_SERVICES],
}

impl LaunchPlan<'_> {
    pub const fn service_count(&self) -> usize {
        self.service_count
    }

    pub const fn initial_vm_image(&self) -> Option<&str> {
        self.initial_vm_image
    }

    pub fn service_index(&self, launch_position: usize) -> Option<usize> {
        self.order
            .get(launch_position)
            .copied()
            .filter(|_| launch_position < self.service_count)
    }

    /// Returns one complete resolved row or no row at all.
    pub fn capability_grant(&self, service: usize, capability: usize) -> Option<CapabilityGrant> {
        if service >= self.service_count || capability >= MAX_CAPABILITIES_PER_SERVICE {
            return None;
        }
        self.grants[service][capability]
    }

    /// Finds the only service which receives `purpose`.
    ///
    /// Singleton service roles are bound to a validated capability contract,
    /// not to a mutable executable path. Zero or ambiguous matches are both
    /// rejected so callers cannot accidentally provision the wrong process.
    pub fn unique_service_for_purpose(&self, purpose: u32) -> Option<usize> {
        if purpose == 0 {
            return None;
        }
        let mut selected = None;
        for service in 0..self.service_count {
            if self.grants[service]
                .iter()
                .flatten()
                .any(|grant| grant.purpose() == purpose)
            {
                if selected.is_some() {
                    return None;
                }
                selected = Some(service);
            }
        }
        selected
    }
}

/// Validates the complete graph before any `ProcessBuilder` may start a service.
pub fn validate<'manifest>(
    manifest: &Manifest<'manifest>,
    policy: &impl AuthorityPolicy,
) -> Result<LaunchPlan<'manifest>, ValidationError> {
    if manifest.services.is_empty() {
        return Err(error(ValidationErrorKind::EmptyManifest, None, None));
    }
    validate_service_identities(manifest)?;
    validate_initial_vm(manifest)?;
    validate_dependencies(manifest)?;

    let mut plan = LaunchPlan {
        service_count: manifest.services.len(),
        initial_vm_image: manifest.initial_vm_image(),
        order: [0; MAX_SERVICES],
        grants: [[None; MAX_CAPABILITIES_PER_SERVICE]; MAX_SERVICES],
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

fn validate_initial_vm(manifest: &Manifest<'_>) -> Result<(), ValidationError> {
    if manifest
        .initial_vm()
        .is_some_and(|initial_vm| !valid_image_path(initial_vm.image(), MAX_IMAGE_PATH_BYTES))
    {
        return Err(error(
            ValidationErrorKind::InvalidInitialVmImage,
            None,
            None,
        ));
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
    plan: &mut LaunchPlan<'_>,
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
            reject_conflicting_authority_key(
                manifest,
                plan,
                service_index,
                capability_index,
                capability.source,
                declaration.key,
            )?;
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
            if purpose.value == 0
                || purpose.object_kind == 0
                || purpose.required_rights & !purpose.allowed_rights != 0
            {
                return Err(error(
                    ValidationErrorKind::InvalidPurposeDeclaration,
                    Some(service_index),
                    Some(capability_index),
                ));
            }
            if plan.grants[service_index][..capability_index]
                .iter()
                .flatten()
                .any(|grant| grant.purpose() == purpose.value)
            {
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
            if purpose.required_rights & !requested_rights != 0 {
                return Err(error(
                    ValidationErrorKind::MissingRequiredRights,
                    Some(service_index),
                    Some(capability_index),
                ));
            }
            if requested_rights & !purpose.allowed_rights != 0 {
                return Err(error(
                    ValidationErrorKind::ExcessPurposeRights,
                    Some(service_index),
                    Some(capability_index),
                ));
            }
            plan.grants[service_index][capability_index] = Some(CapabilityGrant {
                authority: declaration.key,
                operation: capability.operation,
                object_kind: purpose.object_kind,
                purpose: purpose.value,
                rights: requested_rights,
            });
        }
    }
    Ok(())
}

fn reject_conflicting_authority_key(
    manifest: &Manifest<'_>,
    plan: &LaunchPlan<'_>,
    service_index: usize,
    capability_index: usize,
    source: &str,
    key: AuthorityKey,
) -> Result<(), ValidationError> {
    for previous_service in 0..=service_index {
        let Some(service) = manifest.services.get(previous_service) else {
            return Err(error(
                ValidationErrorKind::ConflictingAuthorityKey,
                Some(service_index),
                Some(capability_index),
            ));
        };
        let limit = if previous_service == service_index {
            capability_index
        } else {
            service.capabilities.len()
        };
        for previous_capability in 0..limit {
            if plan.grants[previous_service][previous_capability].map(CapabilityGrant::authority)
                != Some(key)
            {
                continue;
            }
            let Some(previous) = service.capabilities.get(previous_capability) else {
                return Err(error(
                    ValidationErrorKind::ConflictingAuthorityKey,
                    Some(service_index),
                    Some(capability_index),
                ));
            };
            if previous.source != source {
                return Err(error(
                    ValidationErrorKind::ConflictingAuthorityKey,
                    Some(service_index),
                    Some(capability_index),
                ));
            }
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
    plan: &mut LaunchPlan<'_>,
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
