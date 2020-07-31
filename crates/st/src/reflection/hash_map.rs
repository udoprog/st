use crate::reflection::{FromValue, ToValue};
use crate::value::ValuePtr;
use crate::vm::{StackError, Vm};
use std::collections::HashMap;

impl<T> FromValue for HashMap<String, T>
where
    T: FromValue,
{
    fn from_value(value: ValuePtr, vm: &mut Vm) -> Result<Self, StackError> {
        let slot = value.into_array()?;
        let object = vm.object_take(slot)?;

        let mut output = HashMap::with_capacity(object.len());

        for (key, value) in object {
            output.insert(key, T::from_value(value, vm)?);
        }

        Ok(output)
    }
}

impl<T> ToValue for HashMap<String, T>
where
    T: ToValue,
{
    fn to_value(self, vm: &mut Vm) -> Result<ValuePtr, StackError> {
        let mut object = crate::collections::HashMap::with_capacity(self.len());

        for (key, value) in self {
            object.insert(key, value.to_value(vm)?);
        }

        Ok(vm.object_allocate(object))
    }
}