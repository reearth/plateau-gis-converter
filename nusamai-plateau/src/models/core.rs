use nusamai_citygml::{citygml_data, CityGmlElement};

#[citygml_data(name = "core:Address")]
#[citygml(allow_extra)]
pub struct Address {
    #[citygml(path = b"core:xalAddress/xAL:AddressDetails")]
    country: Vec<AddressDetail>,
}

#[citygml_data(name = "xAL:AddressDetails")]
#[citygml(allow_extra)]
pub struct AddressDetail {
    #[citygml(path = b"xAL:Country")]
    country: Option<Country>,
}

#[citygml_data(name = "xAL:Country")]
#[citygml(allow_extra)]
pub struct Country {
    #[citygml(path = b"xAL:CountryName")]
    name: Option<String>,
    #[citygml(path = b"xAL:Locality")]
    locality: Vec<Locality>,
}

#[citygml_data(name = "xAL:Locality")]
#[citygml(allow_extra)]
pub struct Locality {
    #[citygml(path = b"xAL:LocalityName")]
    locality_name: Vec<String>,
    #[citygml(path = b"xAL:DependentLocality")]
    dependent_locality: Vec<DependentLocality>,
}

#[citygml_data(name = "xAL:DependentLocality")]
#[citygml(allow_extra)]
pub struct DependentLocality {
    #[citygml(path = b"@Type")]
    locality_type: Option<String>,
    #[citygml(path = b"xAL:DependentLocalityName")]
    dependent_locality_name: Vec<String>,
}
