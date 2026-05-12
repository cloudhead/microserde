use proc_macro2::Span;
use syn::{Attribute, Error, Field, Lit, Meta, NestedMeta, Result, Variant};

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
