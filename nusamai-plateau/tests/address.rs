//! `core:Address` / xAL parsing.
//!
//! `xAL:Locality` holds the `xAL:LocalityName`s and the `xAL:DependentLocality` (with its `Type`
//! attribute and `xAL:DependentLocalityName`). The model must keep all of it.

use nusamai_citygml::{CityGmlElement, CityGmlReader, ParseContext, Value};
use nusamai_plateau::models::core::Address;

fn parse_address(locality_xml: &str) -> Value {
    let doc = format!(
        r#"<core:Address xmlns:core="http://www.opengis.net/citygml/2.0" xmlns:xAL="urn:oasis:names:tc:ciq:xsdschema:xAL:2.0">
            <core:xalAddress>
                <xAL:AddressDetails>
                    <xAL:Country>
                        <xAL:CountryName>日本</xAL:CountryName>
                        {locality_xml}
                    </xAL:Country>
                </xAL:AddressDetails>
            </core:xalAddress>
        </core:Address>"#
    );
    let mut reader = quick_xml::NsReader::from_str(&doc);
    let mut citygml_reader = CityGmlReader::new(ParseContext::default());
    let mut st = citygml_reader.start_root(&mut reader).unwrap();
    let mut address = Address::default();
    address.parse(&mut st).expect("address should parse");
    address.into_object().expect("address should have a value")
}

fn attr<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    match value {
        Value::Object(obj) => obj.attributes.get(key),
        _ => None,
    }
}

/// A repeatable property with exactly one occurrence.
fn only(value: &Value) -> &Value {
    match value {
        Value::Array(arr) => {
            assert_eq!(arr.len(), 1, "expected exactly one element in {arr:?}");
            &arr[0]
        }
        other => other,
    }
}

fn strings(value: &Value) -> Vec<&str> {
    let Value::Array(arr) = value else {
        panic!("expected an array of strings, got {value:?}");
    };
    arr.iter()
        .map(|v| match v {
            Value::String(s) => s.as_str(),
            other => panic!("expected a string, got {other:?}"),
        })
        .collect()
}

/// core:Address -> core:xalAddress -> xAL:AddressDetails -> xAL:Country -> xAL:Locality
fn locality(address: &Value) -> &Value {
    let details = only(attr(address, "core:xalAddress").expect("core:xalAddress"));
    let country = only(attr(details, "xAL:Country").expect("xAL:Country"));
    only(attr(country, "xAL:Locality").expect("xAL:Locality"))
}

#[test]
fn locality_names_and_district_are_kept() {
    // Fukushima-shi: prefecture and city are separate LocalityNames, the district is a
    // DependentLocality
    let address = parse_address(
        r#"<xAL:Locality>
            <xAL:LocalityName Type="prefecture">福島県</xAL:LocalityName>
            <xAL:LocalityName Type="city">福島市</xAL:LocalityName>
            <xAL:DependentLocality Type="district">
                <xAL:DependentLocalityName>下野寺字薬師堂</xAL:DependentLocalityName>
            </xAL:DependentLocality>
        </xAL:Locality>"#,
    );
    let locality = locality(&address);

    assert_eq!(
        strings(attr(locality, "xAL:LocalityName").expect("xAL:LocalityName")),
        ["福島県", "福島市"]
    );

    let dependent = only(attr(locality, "xAL:DependentLocality").expect("xAL:DependentLocality"));
    assert_eq!(
        attr(dependent, "locality_type"),
        Some(&Value::String("district".into()))
    );
    assert_eq!(
        strings(attr(dependent, "xAL:DependentLocalityName").expect("xAL:DependentLocalityName")),
        ["下野寺字薬師堂"]
    );
}

#[test]
fn dependent_locality_number_is_kept() {
    // Example from the official PLATEAU documentation
    let address = parse_address(
        r#"<xAL:Locality>
            <xAL:LocalityName Type="prefecture">東京都</xAL:LocalityName>
            <xAL:LocalityName Type="city">○○区</xAL:LocalityName>
            <xAL:DependentLocality Type="district">
                <xAL:DependentLocalityName>××</xAL:DependentLocalityName>
                <xAL:DependentLocalityNumber>2-2-3</xAL:DependentLocalityNumber>
            </xAL:DependentLocality>
        </xAL:Locality>"#,
    );
    let locality = locality(&address);

    assert_eq!(
        strings(attr(locality, "xAL:LocalityName").expect("xAL:LocalityName")),
        ["東京都", "○○区"]
    );

    let dependent = only(attr(locality, "xAL:DependentLocality").expect("xAL:DependentLocality"));
    assert_eq!(
        attr(dependent, "locality_type"),
        Some(&Value::String("district".into()))
    );
    assert_eq!(
        strings(attr(dependent, "xAL:DependentLocalityName").expect("xAL:DependentLocalityName")),
        ["××"]
    );
    assert_eq!(
        strings(
            attr(dependent, "xAL:DependentLocalityNumber").expect("xAL:DependentLocalityNumber")
        ),
        ["2-2-3"]
    );
}

#[test]
fn single_locality_name_without_district() {
    let address = parse_address(
        r#"<xAL:Locality>
            <xAL:LocalityName Type="Town">東京都新宿区霞ヶ丘町</xAL:LocalityName>
        </xAL:Locality>"#,
    );
    let locality = locality(&address);

    assert_eq!(
        strings(attr(locality, "xAL:LocalityName").expect("xAL:LocalityName")),
        ["東京都新宿区霞ヶ丘町"]
    );
    assert!(attr(locality, "xAL:DependentLocality").is_none());
}

#[test]
fn dependent_locality_types_are_kept() {
    let address = parse_address(
        r#"<xAL:Locality>
            <xAL:LocalityName>福島県</xAL:LocalityName>
            <xAL:DependentLocality Type="other">
                <xAL:DependentLocalityName>その他</xAL:DependentLocalityName>
            </xAL:DependentLocality>
            <xAL:DependentLocality Type="district">
                <xAL:DependentLocalityName>下野寺字薬師堂</xAL:DependentLocalityName>
            </xAL:DependentLocality>
        </xAL:Locality>"#,
    );
    let locality = locality(&address);

    let Some(Value::Array(dependents)) = attr(locality, "xAL:DependentLocality") else {
        panic!("expected DependentLocality array");
    };
    let got: Vec<(Option<&Value>, Vec<&str>)> = dependents
        .iter()
        .map(|d| {
            (
                attr(d, "locality_type"),
                strings(attr(d, "xAL:DependentLocalityName").unwrap()),
            )
        })
        .collect();
    assert_eq!(
        got,
        [
            (Some(&Value::String("other".into())), vec!["その他"]),
            (
                Some(&Value::String("district".into())),
                vec!["下野寺字薬師堂"]
            ),
        ]
    );
}

#[test]
fn unrelated_xal_elements_are_ignored() {
    // xAL elements the model does not describe must not fail the parse
    let address = parse_address(
        r#"<xAL:Locality>
            <xAL:LocalityName>福島市</xAL:LocalityName>
            <xAL:PostalCode><xAL:PostalCodeNumber>960-8000</xAL:PostalCodeNumber></xAL:PostalCode>
            <xAL:Thoroughfare><xAL:ThoroughfareName>1-1</xAL:ThoroughfareName></xAL:Thoroughfare>
        </xAL:Locality>"#,
    );
    let locality = locality(&address);

    assert_eq!(
        strings(attr(locality, "xAL:LocalityName").expect("xAL:LocalityName")),
        ["福島市"]
    );
}
