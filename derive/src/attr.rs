use proc_macro2::Span;
use syn::{
    Attribute, DataStruct, Error, Field, Fields, FieldsNamed, FieldsUnnamed, Lit, Meta, NestedMeta,
    Result, Variant,
};

/// Supported `#[serde(...)]` attributes on a struct.
pub struct StructAttrs {
    /// Whether a one-field struct serializes as its field.
    pub transparent: bool,
}

/// Supported struct representation.
pub enum StructKind<'a> {
    /// `#[serde(transparent)]` tuple newtype.
    Transparent(&'a FieldsUnnamed),
    /// Ordinary named-field struct.
    Named(&'a FieldsNamed),
}

/// Find supported #[serde(...)] struct attributes.
pub fn struct_attrs(attrs: &[Attribute]) -> Result<StructAttrs> {
    let mut transparent = false;

    for attr in attrs {
        if !attr.path.is_ident("serde") {
            continue;
        }

        let list = match attr.parse_meta()? {
            Meta::List(list) => list,
            other => return Err(Error::new_spanned(other, "unsupported attribute")),
        };

        for meta in &list.nested {
            if let NestedMeta::Meta(Meta::Path(path)) = meta {
                if path.is_ident("transparent") {
                    if transparent {
                        return Err(Error::new_spanned(meta, "duplicate transparent attribute"));
                    }
                    transparent = true;
                    continue;
                }
            }
            return Err(Error::new_spanned(meta, "unsupported attribute"));
        }
    }
    Ok(StructAttrs { transparent })
}

/// Classify a struct according to supported serde attributes.
pub fn struct_kind<'a>(attrs: &[Attribute], input: &'a DataStruct) -> Result<StructKind<'a>> {
    let attrs = struct_attrs(attrs)?;
    if attrs.transparent {
        match &input.fields {
            Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                Ok(StructKind::Transparent(fields))
            }
            _ => Err(Error::new_spanned(
                &input.fields,
                "transparent structs must be tuple structs with exactly one field",
            )),
        }
    } else if let Fields::Named(fields) = &input.fields {
        Ok(StructKind::Named(fields))
    } else {
        Err(Error::new_spanned(
            &input.fields,
            "currently only structs with named fields are supported",
        ))
    }
}

/// Supported `#[serde(...)]` attributes on a struct field.
pub struct FieldAttrs {
    /// Field name override from `#[serde(rename = "...")]`.
    pub rename: Option<String>,
    /// Whether the field uses `#[serde(default)]` when missing.
    pub default: bool,
}

/// Find supported #[serde(...)] field attributes.
pub fn field_attrs(attrs: &[Attribute]) -> Result<FieldAttrs> {
    let mut rename = None;
    let mut default = false;

    for attr in attrs {
        if !attr.path.is_ident("serde") {
            continue;
        }

        let list = match attr.parse_meta()? {
            Meta::List(list) => list,
            other => return Err(Error::new_spanned(other, "unsupported attribute")),
        };

        for meta in &list.nested {
            if let NestedMeta::Meta(Meta::NameValue(value)) = meta {
                if value.path.is_ident("rename") {
                    if let Lit::Str(s) = &value.lit {
                        if rename.is_some() {
                            return Err(Error::new_spanned(meta, "duplicate rename attribute"));
                        }
                        rename = Some(s.value());
                        continue;
                    }
                }
            }
            if let NestedMeta::Meta(Meta::Path(path)) = meta {
                if path.is_ident("default") {
                    if default {
                        return Err(Error::new_spanned(meta, "duplicate default attribute"));
                    }
                    default = true;
                    continue;
                }
            }
            return Err(Error::new_spanned(meta, "unsupported attribute"));
        }
    }

    Ok(FieldAttrs { rename, default })
}

/// Find the value of a #[serde(rename = "...")] attribute.
fn attr_rename(attrs: &[Attribute]) -> Result<Option<String>> {
    let attrs = field_attrs(attrs)?;
    if attrs.default {
        return Err(Error::new(
            Span::call_site(),
            "default attribute is only supported on fields",
        ));
    }
    Ok(attrs.rename)
}

/// Determine the name of a field, respecting a rename attribute.
pub fn name_of_field(field: &Field) -> Result<String> {
    let attrs = field_attrs(&field.attrs)?;
    Ok(attrs
        .rename
        .unwrap_or_else(|| field.ident.as_ref().unwrap().to_string()))
}

/// Determine the name of a variant, respecting a rename attribute.
pub fn name_of_variant(var: &Variant) -> Result<String> {
    let rename = attr_rename(&var.attrs)?;
    Ok(rename.unwrap_or_else(|| var.ident.to_string()))
}
