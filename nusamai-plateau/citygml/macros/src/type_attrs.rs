extern crate proc_macro;

use proc_macro2::TokenStream;
use quote::quote;
use syn::meta::ParseNestedMeta;
use syn::{parse::Parser, parse_macro_input, Data, DeriveInput, Error, LitStr};
use syn::{parse_quote, LitByteStr};

#[derive(Default)]
struct FeatureArgs {
    name: Option<LitStr>,       // "bldg:Building"
    prefix: Option<LitByteStr>, // b"bldg"
}

impl FeatureArgs {
    fn parse(&mut self, meta: ParseNestedMeta) -> syn::parse::Result<()> {
        if meta.path.is_ident("name") {
            let s: LitStr = meta.value()?.parse()?;
            self.prefix = Some(LitByteStr::new(
                s.value().split_once(':').unwrap().0.as_bytes(),
                s.span(),
            ));
            self.name = Some(s);
            Ok(())
        } else {
            Err(meta.error("unsupported property"))
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum ElementType {
    Feature,
    Data,
    Property,
}

pub(crate) fn citygml_type(
    ty: ElementType,
    args: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let mut attrs = FeatureArgs::default();
    let tea_parser = syn::meta::parser(|meta| attrs.parse(meta));
    parse_macro_input!(args with tea_parser);

    let mut input = parse_macro_input!(input as DeriveInput);

    if let Err(e) = modify(&ty, &attrs, &mut input) {
        return e.to_compile_error().into();
    };

    quote! {
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(tag = "type"))]
        #[derive(Default, Debug, citygml::CityGMLElement)]
        #input
    }
    .into()
}

fn add_named_field(fields: &mut syn::FieldsNamed, body: TokenStream) {
    fields
        .named
        .push(syn::Field::parse_named.parse2(body).unwrap())
}

fn modify(ty: &ElementType, args: &FeatureArgs, input: &mut DeriveInput) -> Result<(), Error> {
    match &args.name {
        Some(name) => {
            input.attrs.push(syn::parse_quote! {
                #[citygml(name = #name)]
            });
        }
        None => return Err(Error::new_spanned(input, "name is required")),
    };

    match &mut input.data {
        Data::Struct(data) => {
            // #[citygml_feature], #[citygml_data]

            match ty {
                ElementType::Feature | ElementType::Data => {}
                _ => return Err(Error::new_spanned(input, "target must be struct")),
            }

            if let syn::Fields::Named(ref mut fields) = data.fields {
                if let ElementType::Feature = ty {
                    // #[citygml_feature]

                    let prefix = args.prefix.as_ref().unwrap();
                    add_named_field(
                        fields,
                        quote! {
                            #[citygml(geom = #prefix)]
                            pub geometries: citygml::GeometryRef
                        },
                    );
                    add_named_field(
                        fields,
                        quote! {
                            #[citygml(path = b"@gml:id")]
                            pub id: Option<String>
                        },
                    );
                    //// CityGML 3.0
                    // add_named_field(
                    //     fields,
                    //     quote! {
                    //         #[citygml(path = b"gml:identifier")]
                    //         pub id: Option<String>
                    //     },
                    // );
                    add_named_field(
                        fields,
                        quote! {
                            #[citygml(path = b"gml:name")]
                            pub name: Vec<String>
                        },
                    );
                    add_named_field(
                        fields,
                        quote! {
                            #[citygml(path = b"gml:description")]
                            pub description: Option<String>
                        },
                    );
                    add_named_field(
                        fields,
                        quote! {
                            #[citygml(path = b"gml:creationDate")]
                            pub creation_date: Option<citygml::Date> // TODO: DateTime (CityGML 3.0)
                        },
                    );
                    add_named_field(
                        fields,
                        quote! {
                            #[citygml(path = b"gml:terminationDate")]
                            pub termination_date: Option<citygml::Date> // TODO: DateTime (CityGML 3.0)
                        },
                    );
                    add_named_field(
                        fields,
                        quote! {
                            #[citygml(path = b"gml:validFrom")]
                            pub valid_from: Option<citygml::Date> // TODO: DateTime (CityGML 3.0)
                        },
                    );
                    add_named_field(
                        fields,
                        quote! {
                            #[citygml(path = b"gml:validTo")]
                            pub valid_to: Option<citygml::Date> // TODO: DateTime (CityGML 3.0)
                        },
                    );
                }
            }
        }
        Data::Enum(_data) => match ty {
            ElementType::Property => {
                // #[citygml_property]
                _data.variants.push(parse_quote! {
                    #[default]
                    Unknown
                });
            }
            _ => return Err(Error::new_spanned(input, "target must be enum")),
        },
        _ => return Err(Error::new_spanned(input, "target must be struct or enum")),
    }
    Ok(())
}