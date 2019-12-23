use byteorder::ByteOrder;
use core::convert::TryInto;
use std::borrow::Cow;

use crate::EncodingContext;
use crate::{SharedData, SimpleVariantType};
use crate::{Variant, VariantError, VariantType, VariantTypeConstants};

// Since neither `From` trait nor `Vec` is from this crate, we need this intermediate type.
//
#[derive(Debug, Clone)]
pub struct Array(Vec<Variant>);

impl Array {
    pub fn new() -> Self {
        Array(vec![])
    }

    pub fn new_from_vec(vec: Vec<Variant>) -> Self {
        Array(vec)
    }

    pub fn inner(&self) -> &Vec<Variant> {
        &self.0
    }

    pub fn inner_mut(&mut self) -> &mut Vec<Variant> {
        &mut self.0
    }

    pub fn take_inner(self) -> Vec<Variant> {
        self.0
    }
}

impl VariantTypeConstants for Array {
    const SIGNATURE_CHAR: char = 'a';
    const SIGNATURE_STR: &'static str = "a";
    const ALIGNMENT: usize = 4;
}

impl VariantType for Array {
    fn signature_char() -> char {
        'a'
    }
    fn signature_str() -> &'static str {
        Self::SIGNATURE_STR
    }
    fn alignment() -> usize {
        Self::ALIGNMENT
    }

    fn encode_into(&self, bytes: &mut Vec<u8>, context: EncodingContext) {
        Self::add_padding(bytes, context);

        let len_position = bytes.len();
        bytes.extend(&0u32.to_ne_bytes());
        let n_bytes_before = bytes.len();
        let child_enc_context = context.copy_for_child();
        for element in self.inner() {
            // Deep copying, nice!!! 🙈
            element.encode_value_into(bytes, child_enc_context);
        }

        // Set size of array in bytes
        let len = crate::utils::usize_to_u32(bytes.len() - n_bytes_before);
        byteorder::NativeEndian::write_u32(&mut bytes[len_position..len_position + 4], len);
    }

    fn slice_data(
        data: &SharedData,
        signature: &str,
        context: EncodingContext,
    ) -> Result<SharedData, VariantError> {
        if signature.len() < 2 {
            return Err(VariantError::InsufficientData);
        }
        Self::ensure_correct_signature(signature)?;

        // Child signature
        let child_signature = crate::variant_type::slice_signature(&signature[1..])?;

        // Array size in bytes
        let len_slice = u32::slice_data_simple(&data, context)?;
        let mut extracted = len_slice.len();
        let len = u32::decode_simple(&len_slice, context)? as usize + 4;
        let child_enc_context = context.copy_for_child();
        while extracted < len {
            let slice = crate::variant_type::slice_data(
                &data.tail(extracted),
                child_signature,
                child_enc_context,
            )?;
            extracted += slice.len();
            if extracted > len {
                return Err(VariantError::InsufficientData);
            }
        }
        if extracted == 0 {
            return Err(VariantError::ExcessData);
        }

        Ok(data.head(extracted as usize))
    }

    fn decode(
        data: &SharedData,
        signature: &str,
        context: EncodingContext,
    ) -> Result<Self, VariantError> {
        let padding = Self::padding(data.position(), context);
        if data.len() < padding + 4 || signature.len() < 2 {
            return Err(VariantError::InsufficientData);
        }
        Self::ensure_correct_signature(signature)?;

        // Child signature
        let child_signature = crate::variant_type::slice_signature(&signature[1..])?;

        // Array size in bytes
        let mut extracted = padding + 4;
        let len = u32::decode_simple(&data.subset(padding, extracted), context)? as usize + 4;
        let child_enc_context = context.copy_for_child();
        let mut elements = vec![];

        while extracted < len {
            let slice = crate::variant_type::slice_data(
                &data.tail(extracted as usize),
                child_signature,
                child_enc_context,
            )?;
            extracted += slice.len();
            if extracted > len {
                return Err(VariantError::InsufficientData);
            }

            let element = Variant::from_data(&slice, child_signature, child_enc_context)?;
            elements.push(element);
        }
        if extracted == 0 {
            return Err(VariantError::ExcessData);
        }

        Ok(Array::new_from_vec(elements))
    }

    fn ensure_correct_signature(signature: &str) -> Result<(), VariantError> {
        let slice = Self::slice_signature(&signature)?;
        if slice.len() != signature.len() {
            return Err(VariantError::IncorrectType);
        }

        Ok(())
    }

    fn signature<'b>(&'b self) -> Cow<'b, str> {
        let signature = format!("a{}", self.inner()[0].value_signature());

        Cow::from(signature)
    }

    fn slice_signature(signature: &str) -> Result<&str, VariantError> {
        if !signature.starts_with("a") {
            return Err(VariantError::IncorrectType);
        }

        // There should be a valid complete signature after 'a' but not more than 1
        let slice = crate::variant_type::slice_signature(&signature[1..])?;

        Ok(&signature[0..slice.len() + 1])
    }

    fn is(variant: &Variant) -> bool {
        if let Variant::Array(_) = variant {
            true
        } else {
            false
        }
    }

    fn take_from_variant(variant: Variant) -> Result<Self, VariantError> {
        if let Variant::Array(value) = variant {
            Ok(value)
        } else {
            Err(VariantError::IncorrectType)
        }
    }

    fn from_variant(variant: &Variant) -> Result<&Self, VariantError> {
        if let Variant::Array(value) = variant {
            Ok(value)
        } else {
            Err(VariantError::IncorrectType)
        }
    }

    fn to_variant(self) -> Variant {
        Variant::Array(self)
    }
}

impl<T: VariantType> TryInto<Vec<T>> for Array {
    type Error = VariantError;

    fn try_into(self) -> Result<Vec<T>, VariantError> {
        let mut v: Vec<T> = vec![];

        for value in self.take_inner() {
            v.push(T::take_from_variant(value)?);
        }

        Ok(v)
    }
}

impl<T: VariantType> From<Vec<T>> for Array {
    fn from(values: Vec<T>) -> Self {
        let mut v: Vec<Variant> = vec![];

        for value in values {
            v.push(value.to_variant());
        }

        Array::new_from_vec(v)
    }
}

impl From<crate::Dict> for Array {
    fn from(value: crate::Dict) -> Self {
        Array::from(value.take_inner())
    }
}
