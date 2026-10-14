use nusamai_citygml::{
    CityGmlElement, CityGmlReader, Date, GeometryRefs, GeometryStore, ParseContext, ParseError,
};

#[derive(CityGmlElement, Default)]
struct Feature {
    #[citygml(path = b"date")]
    date: Option<Date>,
    #[citygml(path = b"name")]
    name: Option<String>,
    #[citygml(geom = b"bldg")]
    geometries: GeometryRefs,
}

// An invalid date, an unknown element, and a lod2MultiSurface whose second polygon has a NaN
const DOC: &str = r#"
    <bldg:Building xmlns:bldg="http://www.opengis.net/citygml/building/2.0" xmlns:gml="http://www.opengis.net/gml">
        <date>2020-13-45</date>
        <unknown><child>1</child></unknown>
        <bldg:lod1MultiSurface>
            <gml:MultiSurface>
                <gml:surfaceMember>
                    <gml:Polygon>
                        <gml:exterior>
                            <gml:LinearRing>
                                <gml:posList>0 0 0 1 0 0 1 1 0 0 1 0 0 0 0</gml:posList>
                            </gml:LinearRing>
                        </gml:exterior>
                    </gml:Polygon>
                </gml:surfaceMember>
            </gml:MultiSurface>
        </bldg:lod1MultiSurface>
        <bldg:lod2MultiSurface>
            <gml:MultiSurface>
                <gml:surfaceMember>
                    <gml:Polygon>
                        <gml:exterior>
                            <gml:LinearRing>
                                <gml:posList>5 5 0 6 5 0 6 6 0 5 6 0 5 5 0</gml:posList>
                            </gml:LinearRing>
                        </gml:exterior>
                    </gml:Polygon>
                </gml:surfaceMember>
                <gml:surfaceMember>
                    <gml:Polygon>
                        <gml:exterior>
                            <gml:LinearRing>
                                <gml:posList>7 7 0 8 7 0 8 8 NaN 7 7 0</gml:posList>
                            </gml:LinearRing>
                        </gml:exterior>
                    </gml:Polygon>
                </gml:surfaceMember>
            </gml:MultiSurface>
        </bldg:lod2MultiSurface>
        <name>kept</name>
    </bldg:Building>
"#;

fn parse(lenient: bool) -> Result<(Feature, GeometryStore), ParseError> {
    let mut reader = quick_xml::NsReader::from_str(DOC);
    let mut citygml_reader = CityGmlReader::new(ParseContext::default().with_lenient(lenient));
    let mut st = citygml_reader.start_root(&mut reader)?;
    let mut feature = Feature::default();
    feature.parse(&mut st)?;
    Ok((feature, st.collect_geometries(None)))
}

#[test]
fn strict_fails_on_first_error() {
    assert!(parse(false).is_err(), "strict mode should fail");
}

#[test]
fn lenient_skips_broken_properties() {
    let (feature, geometries) = parse(true).expect("lenient mode should not fail");

    // the invalid date and the unknown element are skipped, the rest is kept
    assert_eq!(feature.date, None);
    assert_eq!(feature.name, Some("kept".into()));

    // lod2 is dropped as a whole, including its valid first polygon
    assert_eq!(feature.geometries.len(), 1);
    assert_eq!(feature.geometries[0].lod, 1);
    assert_eq!(geometries.multipolygon.len(), 1);
    assert_eq!(geometries.vertices.len(), 4);
    assert_eq!(geometries.ring_ids.len(), 1);
}
