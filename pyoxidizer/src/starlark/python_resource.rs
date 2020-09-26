// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use {
    python_packaging::policy::PythonPackagingPolicy,
    python_packaging::resource::{
        PythonExtensionModule, PythonModuleSource, PythonPackageDistributionResource,
        PythonPackageResource, PythonResource,
    },
    python_packaging::resource_collection::{
        ConcreteResourceLocation, PythonResourceAddCollectionContext,
    },
    starlark::values::error::{
        RuntimeError, UnsupportedOperation, ValueError, INCORRECT_PARAMETER_TYPE_ERROR_CODE,
    },
    starlark::values::none::NoneType,
    starlark::values::{Immutable, Mutable, TypedValue, Value, ValueResult},
    std::convert::{TryFrom, TryInto},
};

#[derive(Clone, Debug)]
pub struct OptionalResourceLocation {
    inner: Option<ConcreteResourceLocation>,
}

impl From<&OptionalResourceLocation> for Value {
    fn from(location: &OptionalResourceLocation) -> Self {
        match &location.inner {
            Some(ConcreteResourceLocation::InMemory) => Value::from("in-memory"),
            Some(ConcreteResourceLocation::RelativePath(prefix)) => {
                Value::from(format!("filesystem-relative:{}", prefix))
            }
            None => Value::from(NoneType::None),
        }
    }
}

impl TryFrom<&str> for OptionalResourceLocation {
    type Error = ValueError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        if s == "default" {
            Ok(OptionalResourceLocation { inner: None })
        } else if s == "in-memory" {
            Ok(OptionalResourceLocation {
                inner: Some(ConcreteResourceLocation::InMemory),
            })
        } else if s.starts_with("filesystem-relative:") {
            let prefix = s.split_at("filesystem-relative:".len()).1;
            Ok(OptionalResourceLocation {
                inner: Some(ConcreteResourceLocation::RelativePath(prefix.to_string())),
            })
        } else {
            Err(ValueError::from(RuntimeError {
                code: INCORRECT_PARAMETER_TYPE_ERROR_CODE,
                message: format!("unable to convert value {} to a resource location", s),
                label: format!(
                    "expected `default`, `in-memory`, or `filesystem-relative:*`; got {}",
                    s
                ),
            }))
        }
    }
}

impl TryFrom<&Value> for OptionalResourceLocation {
    type Error = ValueError;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        match value.get_type() {
            "NoneType" => Ok(OptionalResourceLocation { inner: None }),
            "string" => {
                let s = value.to_str();
                Ok(OptionalResourceLocation::try_from(s.as_str())?)
            }
            t => Err(ValueError::from(RuntimeError {
                code: INCORRECT_PARAMETER_TYPE_ERROR_CODE,
                message: format!("unable to convert value {} to resource location", t),
                label: "resource location conversion".to_string(),
            })),
        }
    }
}

impl Into<Option<ConcreteResourceLocation>> for OptionalResourceLocation {
    fn into(self) -> Option<ConcreteResourceLocation> {
        self.inner
    }
}

/// Defines functionality for exposing `PythonResourceAddCollectionContext` from a type.
pub trait ResourceCollectionContext {
    /// Obtain the `PythonResourceAddCollectionContext` associated with this instance, if available.
    fn add_collection_context(&self) -> &Option<PythonResourceAddCollectionContext>;

    /// Obtain the mutable `PythonResourceAddCollectionContext` associated with this instance, if available.
    fn add_collection_context_mut(&mut self) -> &mut Option<PythonResourceAddCollectionContext>;

    fn as_python_resource(&self) -> PythonResource;

    /// Apply a Python packaging policy to this instance.
    ///
    /// This has the effect of replacing the `PythonResourceAddCollectionContext`
    /// instance with a fresh one derived from the policy. If no context
    /// is currently defined on the instance, a new one will be created so
    /// there is.
    fn apply_packaging_policy(&mut self, policy: &PythonPackagingPolicy) {
        let new_context = policy.derive_collection_add_context(&self.as_python_resource());
        self.add_collection_context_mut().replace(new_context);
    }

    /// Obtains the Starlark object attributes that are defined by the add collection context.
    fn add_collection_context_attrs(&self) -> Vec<&'static str> {
        vec![
            "add_include",
            "add_location",
            "add_location_fallback",
            "add_source",
            "add_bytecode_optimization_level_zero",
            "add_bytecode_optimization_level_one",
            "add_bytecode_optimization_level_two",
        ]
    }

    /// Obtain the attribute value for an add collection context.
    ///
    /// The caller should verify the attribute should be serviced by us
    /// before calling.
    fn get_attr_add_collection_context(&self, attribute: &str) -> ValueResult {
        if !self.add_collection_context_attrs().contains(&attribute) {
            panic!(
                "get_attr_add_collection_context({}) called when it shouldn't have been",
                attribute
            );
        }

        let context = self.add_collection_context();

        Ok(match context {
            Some(context) => match attribute {
                "add_bytecode_optimization_level_zero" => Value::new(context.optimize_level_zero),
                "add_bytecode_optimization_level_one" => Value::new(context.optimize_level_one),
                "add_bytecode_optimization_level_two" => Value::new(context.optimize_level_two),
                "add_include" => Value::new(context.include),
                "add_location" => Value::new::<String>(context.location.clone().into()),
                "add_location_fallback" => match context.location_fallback.as_ref() {
                    Some(location) => Value::new::<String>(location.clone().into()),
                    None => Value::from(NoneType::None),
                },
                "add_source" => Value::new(context.store_source),
                _ => panic!("this should not happen"),
            },
            None => Value::from(NoneType::None),
        })
    }

    fn set_attr_add_collection_context(
        &mut self,
        attribute: &str,
        value: Value,
    ) -> Result<(), ValueError> {
        let context = self.add_collection_context_mut();

        match context {
            Some(context) => {
                match attribute {
                    "add_bytecode_optimization_level_zero" => {
                        context.optimize_level_zero = value.to_bool();
                        Ok(())
                    }
                    "add_bytecode_optimization_level_one" => {
                        context.optimize_level_one = value.to_bool();
                        Ok(())
                    }
                    "add_bytecode_optimization_level_two" => {
                        context.optimize_level_two = value.to_bool();
                        Ok(())
                    }
                    "add_include" => {
                        context.include = value.to_bool();
                        Ok(())
                    }
                    "add_location" => {
                        let location: OptionalResourceLocation = (&value).try_into()?;

                        match location.inner {
                            Some(location) => {
                                context.location = location;

                                Ok(())
                            }
                            None => {
                                Err(ValueError::OperationNotSupported {
                                    op: UnsupportedOperation::SetAttr(attribute.to_string()),
                                    left: "set_attr".to_string(),
                                    right: None,
                                })
                            }
                        }
                    }
                    "add_location_fallback" => {
                        let location: OptionalResourceLocation = (&value).try_into()?;

                        match location.inner {
                            Some(location) => {
                                context.location_fallback = Some(location);
                                Ok(())
                            }
                            None => {
                                context.location_fallback = None;
                                Ok(())
                            }
                        }
                    }
                    "add_source" => {
                        context.store_source = value.to_bool();
                        Ok(())
                    }
                    attr => panic!("set_attr_add_collection_context({}) called when it shouldn't have been", attr)
                }
            },
            None => Err(ValueError::from(RuntimeError {
                code: "PYOXIDIZER",
                message: "attempting to set a collection context attribute on an object without a context".to_string(),
                label: "setattr()".to_string()
            }))
        }
    }
}

/// Starlark value wrapper for `PythonModuleSource`.
#[derive(Debug, Clone)]
pub struct PythonSourceModuleValue {
    pub inner: PythonModuleSource,
    pub add_context: Option<PythonResourceAddCollectionContext>,
}

impl PythonSourceModuleValue {
    pub fn new(module: PythonModuleSource) -> Self {
        Self {
            inner: module,
            add_context: None,
        }
    }
}

impl ResourceCollectionContext for PythonSourceModuleValue {
    fn add_collection_context(&self) -> &Option<PythonResourceAddCollectionContext> {
        &self.add_context
    }

    fn add_collection_context_mut(&mut self) -> &mut Option<PythonResourceAddCollectionContext> {
        &mut self.add_context
    }

    fn as_python_resource(&self) -> PythonResource<'_> {
        PythonResource::from(&self.inner)
    }
}

impl TypedValue for PythonSourceModuleValue {
    type Holder = Mutable<PythonSourceModuleValue>;
    const TYPE: &'static str = "PythonSourceModule";

    fn values_for_descendant_check_and_freeze(&self) -> Box<dyn Iterator<Item = Value>> {
        Box::new(std::iter::empty())
    }

    fn to_str(&self) -> String {
        format!("PythonSourceModule<name={}>", self.inner.name)
    }

    fn to_repr(&self) -> String {
        self.to_str()
    }

    fn get_attr(&self, attribute: &str) -> ValueResult {
        let v = match attribute {
            "name" => Value::new(self.inner.name.clone()),
            "source" => {
                let source = self.inner.source.resolve().map_err(|e| {
                    ValueError::from(RuntimeError {
                        code: "PYOXIDIZER_SOURCE_ERROR",
                        message: format!("error resolving source code: {}", e),
                        label: "source".to_string(),
                    })
                })?;

                let source = String::from_utf8(source).map_err(|_| {
                    ValueError::from(RuntimeError {
                        code: "PYOXIDIZER_SOURCE_ERROR",
                        message: "error converting source code to UTF-8".to_string(),
                        label: "source".to_string(),
                    })
                })?;

                Value::new(source)
            }
            "is_package" => Value::new(self.inner.is_package),
            attr => {
                return if self.add_collection_context_attrs().contains(&attr) {
                    self.get_attr_add_collection_context(attr)
                } else {
                    Err(ValueError::OperationNotSupported {
                        op: UnsupportedOperation::GetAttr(attr.to_string()),
                        left: "PythonSourceModule".to_string(),
                        right: None,
                    })
                };
            }
        };

        Ok(v)
    }

    fn has_attr(&self, attribute: &str) -> Result<bool, ValueError> {
        Ok(match attribute {
            "name" => true,
            "source" => true,
            "is_package" => true,
            attr => self.add_collection_context_attrs().contains(&attr),
        })
    }

    fn set_attr(&mut self, attribute: &str, value: Value) -> Result<(), ValueError> {
        if self.add_collection_context_attrs().contains(&attribute) {
            self.set_attr_add_collection_context(attribute, value)
        } else {
            Err(ValueError::OperationNotSupported {
                op: UnsupportedOperation::SetAttr(attribute.to_string()),
                left: Self::TYPE.to_owned(),
                right: None,
            })
        }
    }
}

/// Starlark `Value` wrapper for `PythonPackageResource`.
#[derive(Debug, Clone)]
pub struct PythonPackageResourceValue {
    pub inner: PythonPackageResource,
    pub add_context: Option<PythonResourceAddCollectionContext>,
}

impl PythonPackageResourceValue {
    pub fn new(resource: PythonPackageResource) -> Self {
        Self {
            inner: resource,
            add_context: None,
        }
    }
}

impl ResourceCollectionContext for PythonPackageResourceValue {
    fn add_collection_context(&self) -> &Option<PythonResourceAddCollectionContext> {
        &self.add_context
    }

    fn add_collection_context_mut(&mut self) -> &mut Option<PythonResourceAddCollectionContext> {
        &mut self.add_context
    }

    fn as_python_resource(&self) -> PythonResource<'_> {
        PythonResource::from(&self.inner)
    }
}

impl TypedValue for PythonPackageResourceValue {
    type Holder = Immutable<PythonPackageResourceValue>;
    const TYPE: &'static str = "PythonPackageResource";

    fn values_for_descendant_check_and_freeze(&self) -> Box<dyn Iterator<Item = Value>> {
        Box::new(std::iter::empty())
    }

    fn to_str(&self) -> String {
        format!(
            "PythonPackageResource<package={}, name={}>",
            self.inner.leaf_package, self.inner.relative_name
        )
    }

    fn to_repr(&self) -> String {
        self.to_str()
    }

    fn get_attr(&self, attribute: &str) -> ValueResult {
        let v = match attribute {
            "package" => Value::new(self.inner.leaf_package.clone()),
            "name" => Value::new(self.inner.relative_name.clone()),
            // TODO expose raw data
            attr => {
                return Err(ValueError::OperationNotSupported {
                    op: UnsupportedOperation::GetAttr(attr.to_string()),
                    left: "PythonPackageResource".to_string(),
                    right: None,
                })
            }
        };

        Ok(v)
    }

    fn has_attr(&self, attribute: &str) -> Result<bool, ValueError> {
        Ok(match attribute {
            "package" => true,
            "name" => true,
            // TODO expose raw data
            _ => false,
        })
    }
}

/// Starlark `Value` wrapper for `PythonPackageDistributionResource`.
#[derive(Debug, Clone)]
pub struct PythonPackageDistributionResourceValue {
    pub inner: PythonPackageDistributionResource,
    pub add_context: Option<PythonResourceAddCollectionContext>,
}

impl PythonPackageDistributionResourceValue {
    pub fn new(resource: PythonPackageDistributionResource) -> Self {
        Self {
            inner: resource,
            add_context: None,
        }
    }
}

impl ResourceCollectionContext for PythonPackageDistributionResourceValue {
    fn add_collection_context(&self) -> &Option<PythonResourceAddCollectionContext> {
        &self.add_context
    }

    fn add_collection_context_mut(&mut self) -> &mut Option<PythonResourceAddCollectionContext> {
        &mut self.add_context
    }

    fn as_python_resource(&self) -> PythonResource<'_> {
        PythonResource::from(&self.inner)
    }
}

impl TypedValue for PythonPackageDistributionResourceValue {
    type Holder = Immutable<PythonPackageDistributionResourceValue>;
    const TYPE: &'static str = "PythonPackageDistributionResource";

    fn values_for_descendant_check_and_freeze(&self) -> Box<dyn Iterator<Item = Value>> {
        Box::new(std::iter::empty())
    }

    fn to_str(&self) -> String {
        format!(
            "PythonPackageDistributionResource<package={}, name={}>",
            self.inner.package, self.inner.name
        )
    }

    fn to_repr(&self) -> String {
        self.to_str()
    }

    fn to_bool(&self) -> bool {
        true
    }

    fn get_attr(&self, attribute: &str) -> ValueResult {
        let v = match attribute {
            "package" => Value::new(self.inner.package.clone()),
            "name" => Value::new(self.inner.name.clone()),
            // TODO expose raw data
            attr => {
                return Err(ValueError::OperationNotSupported {
                    op: UnsupportedOperation::GetAttr(attr.to_string()),
                    left: "PythonPackageDistributionResource".to_string(),
                    right: None,
                })
            }
        };

        Ok(v)
    }

    fn has_attr(&self, attribute: &str) -> Result<bool, ValueError> {
        Ok(match attribute {
            "package" => true,
            "name" => true,
            // TODO expose raw data
            _ => false,
        })
    }
}

/// Starlark `Value` wrapper for `PythonExtensionModule`.
#[derive(Debug, Clone)]
pub struct PythonExtensionModuleValue {
    pub inner: PythonExtensionModule,
}

impl TypedValue for PythonExtensionModuleValue {
    type Holder = Immutable<PythonExtensionModuleValue>;
    const TYPE: &'static str = "PythonExtensionModule";

    fn values_for_descendant_check_and_freeze(&self) -> Box<dyn Iterator<Item = Value>> {
        Box::new(std::iter::empty())
    }

    fn to_str(&self) -> String {
        format!("PythonExtensionModule<name={}>", self.inner.name)
    }

    fn to_repr(&self) -> String {
        self.to_str()
    }

    fn get_attr(&self, attribute: &str) -> ValueResult {
        let v = match attribute {
            "name" => Value::new(self.inner.name.clone()),
            attr => {
                return Err(ValueError::OperationNotSupported {
                    op: UnsupportedOperation::GetAttr(attr.to_string()),
                    left: "PythonExtensionModule".to_string(),
                    right: None,
                })
            }
        };

        Ok(v)
    }

    fn has_attr(&self, attribute: &str) -> Result<bool, ValueError> {
        Ok(match attribute {
            "name" => true,
            _ => false,
        })
    }
}

/// Whether a `PythonResource` can be converted to a Starlark value.
pub fn is_resource_starlark_compatible(resource: &PythonResource) -> bool {
    match resource {
        PythonResource::ModuleSource(_) => true,
        PythonResource::PackageResource(_) => true,
        PythonResource::PackageDistributionResource(_) => true,
        PythonResource::ExtensionModule(_) => true,
        _ => false,
    }
}

pub fn python_resource_to_value(
    resource: &PythonResource,
    policy: &PythonPackagingPolicy,
) -> Value {
    match resource {
        PythonResource::ModuleSource(sm) => {
            let mut m = PythonSourceModuleValue::new(sm.clone().into_owned());
            m.apply_packaging_policy(policy);

            Value::new(m)
        }

        PythonResource::PackageResource(data) => {
            let mut r = PythonPackageResourceValue::new(data.clone().into_owned());
            r.apply_packaging_policy(policy);

            Value::new(r)
        }

        PythonResource::PackageDistributionResource(resource) => {
            let mut r = PythonPackageDistributionResourceValue::new(resource.clone().into_owned());
            r.apply_packaging_policy(policy);

            Value::new(r)
        }

        PythonResource::ExtensionModule(em) => Value::new(PythonExtensionModuleValue {
            inner: em.clone().into_owned(),
        }),

        _ => {
            panic!("incompatible PythonResource variant passed; did you forget to filter through is_resource_starlark_compatible()?")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::*;
    use super::*;

    #[test]
    fn test_source_module_attrs() {
        let (mut env, type_values) = starlark_make_exe().unwrap();

        let mut m = starlark_eval_in_env(
            &mut env,
            &type_values,
            "exe.make_python_source_module('foo', 'import bar')",
        )
        .unwrap();

        assert_eq!(m.get_type(), "PythonSourceModule");
        assert!(m.has_attr("name").unwrap());
        assert_eq!(m.get_attr("name").unwrap().to_str(), "foo");

        assert!(m.has_attr("source").unwrap());
        assert_eq!(m.get_attr("source").unwrap().to_str(), "import bar");

        assert!(m.has_attr("is_package").unwrap());
        assert_eq!(m.get_attr("is_package").unwrap().to_bool(), false);

        assert!(m.has_attr("add_include").unwrap());
        assert_eq!(m.get_attr("add_include").unwrap().get_type(), "bool");
        assert_eq!(m.get_attr("add_include").unwrap().to_bool(), true);
        m.set_attr("add_include", Value::new(false)).unwrap();
        assert_eq!(m.get_attr("add_include").unwrap().to_bool(), false);

        assert!(m.has_attr("add_location").unwrap());
        assert_eq!(m.get_attr("add_location").unwrap().to_str(), "in-memory");

        m.set_attr("add_location", Value::from("in-memory"))
            .unwrap();
        assert_eq!(m.get_attr("add_location").unwrap().to_str(), "in-memory");

        m.set_attr("add_location", Value::from("filesystem-relative:lib"))
            .unwrap();
        assert_eq!(
            m.get_attr("add_location").unwrap().to_str(),
            "filesystem-relative:lib"
        );

        assert!(m.has_attr("add_location_fallback").unwrap());
        assert_eq!(
            m.get_attr("add_location_fallback").unwrap().get_type(),
            "NoneType"
        );

        m.set_attr("add_location_fallback", Value::from("in-memory"))
            .unwrap();
        assert_eq!(
            m.get_attr("add_location_fallback").unwrap().to_str(),
            "in-memory"
        );

        m.set_attr(
            "add_location_fallback",
            Value::from("filesystem-relative:lib"),
        )
        .unwrap();
        assert_eq!(
            m.get_attr("add_location_fallback").unwrap().to_str(),
            "filesystem-relative:lib"
        );

        m.set_attr("add_location_fallback", Value::from(NoneType::None))
            .unwrap();
        assert_eq!(
            m.get_attr("add_location_fallback").unwrap().get_type(),
            "NoneType"
        );

        assert!(m.has_attr("add_source").unwrap());
        assert_eq!(m.get_attr("add_source").unwrap().get_type(), "bool");
        assert_eq!(m.get_attr("add_source").unwrap().to_bool(), true);
        m.set_attr("add_source", Value::new(false)).unwrap();
        assert_eq!(m.get_attr("add_source").unwrap().to_bool(), false);

        assert!(m.has_attr("add_bytecode_optimization_level_zero").unwrap());
        assert_eq!(
            m.get_attr("add_bytecode_optimization_level_zero")
                .unwrap()
                .get_type(),
            "bool"
        );
        assert_eq!(
            m.get_attr("add_bytecode_optimization_level_zero")
                .unwrap()
                .to_bool(),
            true
        );
        m.set_attr("add_bytecode_optimization_level_zero", Value::new(false))
            .unwrap();
        assert_eq!(
            m.get_attr("add_bytecode_optimization_level_zero")
                .unwrap()
                .to_bool(),
            false
        );

        assert!(m.has_attr("add_bytecode_optimization_level_one").unwrap());
        assert_eq!(
            m.get_attr("add_bytecode_optimization_level_one")
                .unwrap()
                .get_type(),
            "bool"
        );
        assert_eq!(
            m.get_attr("add_bytecode_optimization_level_one")
                .unwrap()
                .to_bool(),
            false
        );
        m.set_attr("add_bytecode_optimization_level_one", Value::new(true))
            .unwrap();
        assert_eq!(
            m.get_attr("add_bytecode_optimization_level_one")
                .unwrap()
                .to_bool(),
            true
        );

        assert!(m.has_attr("add_bytecode_optimization_level_two").unwrap());
        assert_eq!(
            m.get_attr("add_bytecode_optimization_level_two")
                .unwrap()
                .get_type(),
            "bool"
        );
        assert_eq!(
            m.get_attr("add_bytecode_optimization_level_two")
                .unwrap()
                .to_bool(),
            false
        );
        m.set_attr("add_bytecode_optimization_level_two", Value::new(true))
            .unwrap();
        assert_eq!(
            m.get_attr("add_bytecode_optimization_level_two")
                .unwrap()
                .to_bool(),
            true
        );
    }
}
