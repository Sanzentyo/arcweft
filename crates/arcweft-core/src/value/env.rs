use super::{RuntimeBinding, RuntimeEnv, RuntimeExactInteger, RuntimeScope, RuntimeValue};

impl Default for RuntimeEnv {
    fn default() -> Self {
        Self {
            scopes: vec![RuntimeScope::default()],
            spare_scopes: Vec::new(),
        }
    }
}

impl Clone for RuntimeEnv {
    fn clone(&self) -> Self {
        Self {
            scopes: self.scopes.clone(),
            spare_scopes: Vec::new(),
        }
    }
}

impl PartialEq for RuntimeEnv {
    fn eq(&self, other: &Self) -> bool {
        self.scopes == other.scopes
    }
}

impl RuntimeEnv {
    pub fn push_scope(&mut self) {
        self.push_scope_with_capacity(0);
    }

    pub(crate) fn push_scope_with_capacity(&mut self, binding_capacity: usize) {
        let mut scope = self
            .spare_scopes
            .pop()
            .unwrap_or_else(|| RuntimeScope::with_capacity(binding_capacity));
        scope.clear();
        scope.reserve_bindings(binding_capacity);
        self.scopes.push(scope);
    }

    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            if let Some(mut scope) = self.scopes.pop() {
                scope.clear();
                self.spare_scopes.push(scope);
            }
        } else if let Some(scope) = self.scopes.last_mut() {
            scope.clear();
        }
    }

    pub fn set(&mut self, name: impl Into<String>, value: RuntimeValue) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.set(name.into(), value);
        }
    }

    pub(crate) fn set_ref(&mut self, name: &str, value: &RuntimeValue) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.set_ref(name, value);
        }
    }

    pub fn set_root(&mut self, name: impl Into<String>, value: RuntimeValue) {
        if let Some(scope) = self.scopes.first_mut() {
            scope.set(name.into(), value);
        }
    }

    pub fn get(&self, name: &str) -> Option<&RuntimeValue> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name))
    }

    pub fn bindings_snapshot(&self) -> Vec<RuntimeBinding> {
        self.scopes
            .iter()
            .flat_map(|scope| scope.bindings.iter().cloned())
            .collect()
    }

    pub(crate) fn replace_scopes_with_bindings(
        &mut self,
        scopes: impl IntoIterator<Item = Vec<RuntimeBinding>>,
    ) {
        self.spare_scopes
            .extend(self.scopes.drain(..).map(|mut scope| {
                scope.clear();
                scope
            }));

        for bindings in scopes {
            let mut scope = self
                .spare_scopes
                .pop()
                .unwrap_or_else(|| RuntimeScope::with_capacity(bindings.len()));
            scope.clear();
            scope.reserve_bindings(bindings.len());
            for binding in bindings {
                scope.set(binding.name, binding.value);
            }
            self.scopes.push(scope);
        }

        if self.scopes.is_empty() {
            self.push_scope();
        }
    }

    pub fn bind_all(&mut self, bindings: impl IntoIterator<Item = RuntimeBinding>) {
        for binding in bindings {
            self.set(binding.name, binding.value);
        }
    }

    pub(crate) fn bind_all_ref(&mut self, bindings: &[RuntimeBinding]) {
        for binding in bindings {
            self.set_ref(&binding.name, &binding.value);
        }
    }

    pub fn bind_all_root(&mut self, bindings: impl IntoIterator<Item = RuntimeBinding>) {
        for binding in bindings {
            self.set_root(binding.name, binding.value);
        }
    }

    pub fn bind_all_root_ref(&mut self, bindings: &[RuntimeBinding]) {
        if self.replace_root_bindings_ref(bindings) {
            return;
        }
        for binding in bindings {
            self.set_root_ref(&binding.name, &binding.value);
        }
    }

    fn replace_root_bindings_ref(&mut self, bindings: &[RuntimeBinding]) -> bool {
        if self.scopes.is_empty() {
            return bindings.is_empty();
        }
        self.scopes
            .first_mut()
            .is_some_and(|scope| scope.replace_binding_values_ref(bindings))
    }

    fn set_root_ref(&mut self, name: &str, value: &RuntimeValue) {
        if self.scopes.is_empty() {
            self.scopes.push(RuntimeScope::default());
        }
        if let Some(scope) = self.scopes.first_mut() {
            scope.set_ref(name, value);
        }
    }

    pub(crate) fn replace_root_i64_bindings(&mut self, input_names: &[String], args: &[i64]) {
        if self.scopes.is_empty() {
            self.scopes.push(RuntimeScope::default());
        }
        self.scopes.truncate(1);
        if let Some(scope) = self.scopes.first_mut() {
            scope.replace_i64_bindings(input_names, args);
        }
    }

    pub(crate) fn replace_root_i32_bindings(&mut self, input_names: &[String], args: &[i32]) {
        if self.scopes.is_empty() {
            self.scopes.push(RuntimeScope::default());
        }
        self.scopes.truncate(1);
        if let Some(scope) = self.scopes.first_mut() {
            scope.replace_i32_bindings(input_names, args);
        }
    }

    pub(crate) fn replace_root_f32_bindings(&mut self, input_names: &[String], args: &[f32]) {
        if self.scopes.is_empty() {
            self.scopes.push(RuntimeScope::default());
        }
        self.scopes.truncate(1);
        if let Some(scope) = self.scopes.first_mut() {
            scope.replace_f32_bindings(input_names, args);
        }
    }

    pub(crate) fn replace_root_f64_bindings(&mut self, input_names: &[String], args: &[f64]) {
        if self.scopes.is_empty() {
            self.scopes.push(RuntimeScope::default());
        }
        self.scopes.truncate(1);
        if let Some(scope) = self.scopes.first_mut() {
            scope.replace_f64_bindings(input_names, args);
        }
    }

    pub(crate) fn replace_root_exact_int_bindings<T: RuntimeExactInteger>(
        &mut self,
        input_names: &[String],
        args: &[T],
    ) {
        if self.scopes.is_empty() {
            self.scopes.push(RuntimeScope::default());
        }
        self.scopes.truncate(1);
        if let Some(scope) = self.scopes.first_mut() {
            scope.replace_exact_int_bindings(input_names, args);
        }
    }

    pub(crate) fn replace_root_value_bindings_ref(
        &mut self,
        input_names: &[String],
        args: &[RuntimeValue],
    ) {
        if self.scopes.is_empty() {
            self.scopes.push(RuntimeScope::default());
        }
        self.scopes.truncate(1);
        if let Some(scope) = self.scopes.first_mut() {
            scope.replace_value_bindings_ref(input_names, args);
        }
    }
}

impl RuntimeScope {
    fn with_capacity(binding_capacity: usize) -> Self {
        Self {
            bindings: Vec::with_capacity(binding_capacity),
        }
    }

    fn reserve_bindings(&mut self, binding_capacity: usize) {
        let additional = binding_capacity.saturating_sub(self.bindings.capacity());
        self.bindings.reserve(additional);
    }

    fn set(&mut self, name: String, value: RuntimeValue) {
        if let Some(binding) = self
            .bindings
            .iter_mut()
            .find(|binding| binding.name == name)
        {
            binding.value = value;
        } else {
            self.bindings.push(RuntimeBinding { name, value });
        }
    }

    fn set_ref(&mut self, name: &str, value: &RuntimeValue) {
        if let Some(binding) = self
            .bindings
            .iter_mut()
            .find(|binding| binding.name == name)
        {
            binding.value = value.clone();
        } else {
            self.bindings.push(RuntimeBinding {
                name: name.to_owned(),
                value: value.clone(),
            });
        }
    }

    fn get(&self, name: &str) -> Option<&RuntimeValue> {
        self.bindings
            .iter()
            .rev()
            .find(|binding| binding.name == name)
            .map(|binding| &binding.value)
    }

    fn clear(&mut self) {
        self.bindings.clear();
    }

    fn replace_i64_bindings(&mut self, input_names: &[String], args: &[i64]) {
        if self.bindings.len() == input_names.len()
            && self
                .bindings
                .iter()
                .zip(input_names)
                .all(|(binding, name)| binding.name == *name)
        {
            self.bindings
                .iter_mut()
                .zip(args.iter().copied())
                .for_each(|(binding, value)| binding.value = RuntimeValue::i64(value));
            return;
        }
        self.bindings.clear();
        self.bindings.extend(
            input_names
                .iter()
                .zip(args.iter().copied())
                .map(|(name, value)| RuntimeBinding {
                    name: name.clone(),
                    value: RuntimeValue::i64(value),
                }),
        );
    }

    fn replace_i32_bindings(&mut self, input_names: &[String], args: &[i32]) {
        if self.bindings.len() == input_names.len()
            && self
                .bindings
                .iter()
                .zip(input_names)
                .all(|(binding, name)| binding.name == *name)
        {
            self.bindings
                .iter_mut()
                .zip(args.iter().copied())
                .for_each(|(binding, value)| binding.value = RuntimeValue::i32(value));
            return;
        }
        self.bindings.clear();
        self.bindings.extend(
            input_names
                .iter()
                .zip(args.iter().copied())
                .map(|(name, value)| RuntimeBinding {
                    name: name.clone(),
                    value: RuntimeValue::i32(value),
                }),
        );
    }

    fn replace_f32_bindings(&mut self, input_names: &[String], args: &[f32]) {
        self.replace_float_bindings(input_names, args, RuntimeValue::F32);
    }

    fn replace_f64_bindings(&mut self, input_names: &[String], args: &[f64]) {
        self.replace_float_bindings(input_names, args, RuntimeValue::F64);
    }

    fn replace_float_bindings<T: Copy>(
        &mut self,
        input_names: &[String],
        args: &[T],
        wrap: impl Fn(T) -> RuntimeValue,
    ) {
        if self.bindings.len() == input_names.len()
            && self
                .bindings
                .iter()
                .zip(input_names)
                .all(|(binding, name)| binding.name == *name)
        {
            self.bindings
                .iter_mut()
                .zip(args.iter().copied())
                .for_each(|(binding, value)| binding.value = wrap(value));
            return;
        }
        self.bindings.clear();
        self.bindings.extend(
            input_names
                .iter()
                .zip(args.iter().copied())
                .map(|(name, value)| RuntimeBinding {
                    name: name.clone(),
                    value: wrap(value),
                }),
        );
    }

    fn replace_exact_int_bindings<T: RuntimeExactInteger>(
        &mut self,
        input_names: &[String],
        args: &[T],
    ) {
        if self.bindings.len() == input_names.len()
            && self
                .bindings
                .iter()
                .zip(input_names)
                .all(|(binding, name)| binding.name == *name)
        {
            self.bindings
                .iter_mut()
                .zip(args.iter().copied())
                .for_each(|(binding, value)| binding.value = value.into_runtime_value());
            return;
        }
        self.bindings.clear();
        self.bindings.extend(
            input_names
                .iter()
                .zip(args.iter().copied())
                .map(|(name, value)| RuntimeBinding {
                    name: name.clone(),
                    value: value.into_runtime_value(),
                }),
        );
    }

    fn replace_value_bindings_ref(&mut self, input_names: &[String], args: &[RuntimeValue]) {
        if self.bindings.len() == input_names.len()
            && self
                .bindings
                .iter()
                .zip(input_names)
                .all(|(binding, name)| binding.name == *name)
        {
            self.bindings
                .iter_mut()
                .zip(args)
                .for_each(|(binding, value)| binding.value = value.clone());
            return;
        }
        self.bindings.clear();
        self.bindings.extend(
            input_names
                .iter()
                .zip(args)
                .map(|(name, value)| RuntimeBinding {
                    name: name.clone(),
                    value: value.clone(),
                }),
        );
    }

    fn replace_binding_values_ref(&mut self, bindings: &[RuntimeBinding]) -> bool {
        if self.bindings.len() != bindings.len() {
            return false;
        }
        if !self
            .bindings
            .iter()
            .zip(bindings)
            .all(|(current, next)| current.name == next.name)
        {
            return false;
        }
        self.bindings
            .iter_mut()
            .zip(bindings)
            .for_each(|(current, next)| current.value = next.value.clone());
        true
    }
}
