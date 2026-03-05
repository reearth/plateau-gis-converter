use std::collections::HashMap;
use std::fmt::Display;
use std::sync::{Arc, RwLock};

use flatgeom::{MultiLineString, MultiPoint, MultiPolygon};
use nusamai_projection::crs::*;
use url::Url;

use crate::LocalId;

/// URI prefix for EPSG codes
const CRS_URI_EPSG_PREFIX: &str = "http://www.opengis.net/def/crs/EPSG/0/";

#[derive(Debug, Clone, Copy)]
pub enum GeometryParseType {
    Geometry,
    Solid,
    MultiSolid,
    MultiSurface,
    MultiCurve,
    MultiPoint,
    Surface,
    Point,
    Triangulated,
    CompositeCurve,
}

/// GML geometry types as they appear in the XML
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum GmlGeometryType {
    // polygonal types
    MultiSolid,
    Solid,
    MultiSurface,
    CompositeSurface,
    OrientableSurface,
    Polygon,
    Surface,
    TriangulatedSurface,
    Tin,

    // Linear types
    LineString,
    MultiCurve,
    CompositeCurve,

    // Point types
    Point,
    MultiPoint,

    // Generic
    Geometry,
    MultiGeometry,
}

impl GmlGeometryType {
    /// Parse from a string slice (XML element local name)
    /// This function is designed to return an option since whether an object has geometric types depends on the flattening settings.
    pub fn maybe_from_str(s: &str) -> Option<Self> {
        match s {
            "Solid" => Some(Self::Solid),
            "MultiSolid" => Some(Self::MultiSolid),
            "MultiSurface" => Some(Self::MultiSurface),
            "CompositeSurface" => Some(Self::CompositeSurface),
            "OrientableSurface" => Some(Self::OrientableSurface),
            "Polygon" => Some(Self::Polygon),
            "Surface" => Some(Self::Surface),
            "TriangulatedSurface" => Some(Self::TriangulatedSurface),
            "Tin" => Some(Self::Tin),
            "LineString" => Some(Self::LineString),
            "MultiCurve" => Some(Self::MultiCurve),
            "CompositeCurve" => Some(Self::CompositeCurve),
            "Point" => Some(Self::Point),
            "MultiPoint" => Some(Self::MultiPoint),
            "Geometry" => Some(Self::Geometry),
            "MultiGeometry" => Some(Self::MultiGeometry),
            _ => None,
        }
    }
}

impl Display for GmlGeometryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Solid => "Solid",
            Self::MultiSolid => "MultiSolid",
            Self::MultiSurface => "MultiSurface",
            Self::CompositeSurface => "CompositeSurface",
            Self::OrientableSurface => "OrientableSurface",
            Self::Polygon => "Polygon",
            Self::Surface => "Surface",
            Self::TriangulatedSurface => "TriangulatedSurface",
            Self::Tin => "Tin",
            Self::LineString => "LineString",
            Self::MultiCurve => "MultiCurve",
            Self::CompositeCurve => "CompositeCurve",
            Self::Point => "Point",
            Self::MultiPoint => "MultiPoint",
            Self::Geometry => "Geometry",
            Self::MultiGeometry => "MultiGeometry",
        };
        write!(f, "{s}")
    }
}

/// CityGML property names that contain geometry
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum PropertyType {
    // Standard LOD properties
    Lod0Point,
    Lod0MultiCurve,
    Lod1MultiCurve,
    Lod2MultiCurve,
    Lod3MultiCurve,
    Lod4MultiCurve,

    Lod1Solid,
    Lod2Solid,
    Lod3Solid,
    Lod4Solid,

    Lod1MultiSolid,
    Lod2MultiSolid,
    Lod3MultiSolid,
    Lod4MultiSolid,

    Lod0MultiSurface,
    Lod1MultiSurface,
    Lod2MultiSurface,
    Lod3MultiSurface,
    Lod4MultiSurface,

    Lod0Geometry,
    Lod1Geometry,
    Lod2Geometry,
    Lod3Geometry,
    Lod4Geometry,

    // Special properties
    Lod0RoofEdge,
    Lod0FootPrint,
    Lod0Network,
    Lod2Network,
    Lod3Network,
    Lod2Surface,
    Lod3Surface,
    Tin,
}

impl PropertyType {
    /// Parse from a string slice (property name without namespace)
    /// This function is designed to return an option since whether an object has geometric properties depends on the flattening settings.
    pub fn maybe_from_str(s: &str) -> Option<Self> {
        let out = match s {
            "lod0Point" => Self::Lod0Point,
            "lod0MultiCurve" => Self::Lod0MultiCurve,
            "lod1MultiCurve" => Self::Lod1MultiCurve,
            "lod2MultiCurve" => Self::Lod2MultiCurve,
            "lod3MultiCurve" => Self::Lod3MultiCurve,
            "lod4MultiCurve" => Self::Lod4MultiCurve,

            "lod1Solid" => Self::Lod1Solid,
            "lod2Solid" => Self::Lod2Solid,
            "lod3Solid" => Self::Lod3Solid,
            "lod4Solid" => Self::Lod4Solid,

            "lod1MultiSolid" => Self::Lod1MultiSolid,
            "lod2MultiSolid" => Self::Lod2MultiSolid,
            "lod3MultiSolid" => Self::Lod3MultiSolid,
            "lod4MultiSolid" => Self::Lod4MultiSolid,

            "lod0MultiSurface" => Self::Lod0MultiSurface,
            "lod1MultiSurface" => Self::Lod1MultiSurface,
            "lod2MultiSurface" => Self::Lod2MultiSurface,
            "lod3MultiSurface" => Self::Lod3MultiSurface,
            "lod4MultiSurface" => Self::Lod4MultiSurface,

            "lod0Geometry" => Self::Lod0Geometry,
            "lod1Geometry" => Self::Lod1Geometry,
            "lod2Geometry" => Self::Lod2Geometry,
            "lod3Geometry" => Self::Lod3Geometry,
            "lod4Geometry" => Self::Lod4Geometry,

            "lod0RoofEdge" => Self::Lod0RoofEdge,
            "lod0FootPrint" => Self::Lod0FootPrint,
            "lod0Network" => Self::Lod0Network,
            "lod2Network" => Self::Lod2Network,
            "lod3Network" => Self::Lod3Network,
            "lod2Surface" => Self::Lod2Surface,
            "lod3Surface" => Self::Lod3Surface,
            "tin" => Self::Tin,

            &_ => return None,
        };
        Some(out)
    }
}

impl Display for PropertyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Lod0Point => "lod0Point",
            Self::Lod0MultiCurve => "lod0MultiCurve",
            Self::Lod1MultiCurve => "lod1MultiCurve",
            Self::Lod2MultiCurve => "lod2MultiCurve",
            Self::Lod3MultiCurve => "lod3MultiCurve",
            Self::Lod4MultiCurve => "lod4MultiCurve",

            Self::Lod1Solid => "lod1Solid",
            Self::Lod2Solid => "lod2Solid",
            Self::Lod3Solid => "lod3Solid",
            Self::Lod4Solid => "lod4Solid",

            Self::Lod1MultiSolid => "lod1MultiSolid",
            Self::Lod2MultiSolid => "lod2MultiSolid",
            Self::Lod3MultiSolid => "lod3MultiSolid",
            Self::Lod4MultiSolid => "lod4MultiSolid",

            Self::Lod0MultiSurface => "lod0MultiSurface",
            Self::Lod1MultiSurface => "lod1MultiSurface",
            Self::Lod2MultiSurface => "lod2MultiSurface",
            Self::Lod3MultiSurface => "lod3MultiSurface",
            Self::Lod4MultiSurface => "lod4MultiSurface",

            Self::Lod0Geometry => "lod0Geometry",
            Self::Lod1Geometry => "lod1Geometry",
            Self::Lod2Geometry => "lod2Geometry",
            Self::Lod3Geometry => "lod3Geometry",
            Self::Lod4Geometry => "lod4Geometry",

            Self::Lod0RoofEdge => "lod0RoofEdge",
            Self::Lod0FootPrint => "lod0FootPrint",
            Self::Lod0Network => "lod0Network",
            Self::Lod2Network => "lod2Network",
            Self::Lod3Network => "lod3Network",
            Self::Lod2Surface => "lod2Surface",
            Self::Lod3Surface => "lod3Surface",
            Self::Tin => "tin",
        };
        write!(f, "{s}")
    }
}

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum GeometryType {
    /// Polygons (solids)
    Solid,
    /// Polygons (surfaces)
    Surface,
    /// Polygons (triangles)
    Triangle,
    /// Line-strings
    Curve,
    /// Points
    Point,
}

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct GeometryRef {
    pub id: Option<LocalId>,
    #[serde(rename = "type")]
    pub ty: GeometryType,
    /// CityGML property name (e.g., Lod2MultiSurface, Lod1Solid, Lod3Geometry)
    pub property_name: Option<PropertyType>,
    /// GML geometry type (e.g., MultiSurface, Polygon, CompositeSurface)
    pub gml_geometry_type: Option<GmlGeometryType>,
    pub lod: u8,
    pub pos: u32,
    pub len: u32,
    pub feature_id: Option<String>,
    pub feature_type: Option<String>,
    /// Unresolved xlink:href references (optional cross-file URL, polygon ID, flip winding)
    /// None file URL = same-file reference; Some = cross-file reference
    #[serde(default)]
    pub unresolved_refs: Vec<(Option<Url>, LocalId, bool)>,
    /// Resolved polygon ranges from xlink:href references (start, end, flip winding)
    #[serde(default)]
    pub resolved_ranges: Vec<(u32, u32, bool)>,
}

pub type GeometryRefs = Vec<GeometryRef>;

/// Geometries in a single city object and all its children.
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Debug, Default)]
pub struct GeometryStore {
    /// EPSG code of the Coordinate Reference System (CRS) for this geometry
    pub epsg: EpsgCode,

    /// Shared vertex buffer for all geometries in this store
    pub vertices: Vec<[f64; 3]>,

    /// All polygons, referenced by `GeometryRefs`
    pub multipolygon: MultiPolygon<'static, u32>,
    /// All line-strings, referenced by `GeometryRefs`
    pub multilinestring: MultiLineString<'static, u32>,
    /// All points, referenced by `GeometryRefs`
    pub multipoint: MultiPoint<'static, u32>,

    /// Ring ids of the all polygons in flattened order:
    /// multipolygon1: exterior = ring_ids[0], interior[0] = ring_ids[1], ... (N rings)
    /// multipolygon2: exterior = ring_ids[N], interior[0] = ring_ids[N+1], ...
    pub ring_ids: Vec<Option<LocalId>>,
    /// List of surface ids and their spans in `multipolygon`
    pub surface_spans: Vec<SurfaceSpan>,

    /// Assigned materials for each polygon. Empty if appearance resolution is not enabled.
    pub polygon_materials: Vec<Option<u32>>,
    /// Assigned textures for each polygon. Empty if appearance resolution is not enabled.
    pub polygon_textures: Vec<Option<u32>>,
    /// Assigned texture UVs for each polygon. Empty if appearance resolution is not enabled.
    pub polygon_uvs: MultiPolygon<'static, [f64; 2]>,
}

impl GeometryStore {
    /// Returns a prefix-sum of ring counts over `multipolygon`.
    /// `result[i]` is the flat index into `ring_ids` for the first ring of polygon `i`.
    pub fn ring_offset_prefix(&self) -> Vec<usize> {
        let n = self.multipolygon.len();
        let mut p = vec![0usize; n + 1];
        for (i, poly) in self.multipolygon.iter_range(0..n).enumerate() {
            p[i + 1] = p[i] + poly.rings().count();
        }
        p
    }

    /// Returns a map from surface id to `(start, end)` polygon span indices.
    pub fn surface_span_index(&self) -> HashMap<LocalId, (u32, u32)> {
        self.surface_spans
            .iter()
            .map(|s| (s.id.clone(), (s.start, s.end)))
            .collect()
    }

    /// Resolve xlink:href references in GeometryRefs by looking up surface_spans.
    /// After this call, same-file unresolved refs are converted to resolved polygon ranges
    pub fn resolve_refs(&self, geomrefs: &mut GeometryRefs) {
        // Build lookup: LocalId -> (start, end)
        let span_map: std::collections::HashMap<&LocalId, (u32, u32)> = self
            .surface_spans
            .iter()
            .map(|s| (&s.id, (s.start, s.end)))
            .collect();

        for geomref in geomrefs.iter_mut() {
            if geomref.unresolved_refs.is_empty() {
                continue;
            }
            let mut ranges = Vec::new();
            let mut remaining = Vec::new();
            for (file_url, href, flip) in geomref.unresolved_refs.drain(..) {
                if file_url.is_some() {
                    // Cross-file ref: leave for caller to resolve via resolve_cross_file_refs
                    remaining.push((file_url, href, flip));
                } else if let Some(&(start, end)) = span_map.get(&href) {
                    ranges.push((start, end, flip));
                } else {
                    log::warn!(
                        "Warning: GeometryRef has unresolved xlink:href reference to id '{:?}', skipping",
                        href
                    );
                }
            }
            geomref.resolved_ranges.extend(ranges);
            geomref.unresolved_refs = remaining;
        }
    }

    /// Resolve cross-file xlink:href geometry references using a pre-built registry.
    /// The registry maps polygon URL (`{file_url}#{polygon_id}`) to the owning GeometryStore.
    /// Copies polygon geometry, ring_ids, and surface_spans from source stores.
    pub fn resolve_cross_file_refs(
        &mut self,
        geomrefs: &mut GeometryRefs,
        registry: &HashMap<Url, Arc<RwLock<Self>>>,
    ) {
        for geomref in geomrefs.iter_mut() {
            if geomref.unresolved_refs.iter().all(|(f, _, _)| f.is_none()) {
                continue;
            }
            let cross_refs: Vec<_> = geomref.unresolved_refs.drain(..).collect();
            let mut remaining = Vec::new();
            // Per-source cache: ring-count prefix sum + surface-span index.
            let mut src_cache: HashMap<Url, SrcFileCache> = HashMap::new();

            for (file_url_opt, href, flip) in cross_refs {
                let Some(file_url) = file_url_opt else {
                    remaining.push((None, href, flip));
                    continue;
                };
                let mut poly_url = file_url.clone();
                poly_url.set_fragment(Some(&href.0));

                let Some(src_lock) = registry.get(&poly_url) else {
                    log::warn!("Cross-file ref not in registry: {poly_url}");
                    continue;
                };
                let src = src_lock.read().unwrap();

                let cache = src_cache.entry(file_url).or_insert_with(|| SrcFileCache {
                    ring_prefix: src.ring_offset_prefix(),
                    span_index: src.surface_span_index(),
                });

                let Some(&(span_start, span_end)) = cache.span_index.get(&href) else {
                    log::warn!("Polygon {:?} not found in source GeometryStore", href);
                    continue;
                };

                let src_ring_offset = cache.ring_prefix[span_start as usize];

                let poly_begin = self.multipolygon.len() as u32;
                let mut ring_count = 0usize;

                for src_poly in src
                    .multipolygon
                    .iter_range(span_start as usize..span_end as usize)
                {
                    let coord_poly = src_poly.transform(|c| src.vertices[*c as usize]);
                    for (ring_i, ring) in coord_poly.rings().enumerate() {
                        let new_indices: Vec<u32> = ring
                            .iter()
                            .map(|coord| {
                                let idx = self.vertices.len() as u32;
                                self.vertices.push(coord);
                                idx
                            })
                            .collect();
                        if ring_i == 0 {
                            self.multipolygon.add_exterior(new_indices);
                        } else {
                            self.multipolygon.add_interior(new_indices);
                        }
                        self.ring_ids.push(
                            src.ring_ids
                                .get(src_ring_offset + ring_count)
                                .cloned()
                                .flatten(),
                        );
                        ring_count += 1;
                    }
                }

                let poly_end = self.multipolygon.len() as u32;

                for src_span in src
                    .surface_spans
                    .iter()
                    .filter(|s| s.start >= span_start && s.end <= span_end)
                {
                    // dst = src_span.{start,end} - span_start + poly_begin
                    // Compute in two steps (both non-negative) to avoid u32 underflow.
                    let rel_start = src_span.start - span_start;
                    let rel_end = src_span.end - span_start;
                    self.surface_spans.push(SurfaceSpan {
                        id: src_span.id.clone(),
                        start: poly_begin + rel_start,
                        end: poly_begin + rel_end,
                    });
                }

                geomref.resolved_ranges.push((poly_begin, poly_end, flip));
            }

            geomref.unresolved_refs = remaining;
        }
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct SurfaceSpan {
    pub id: LocalId,
    pub start: u32,
    pub end: u32,
}

struct SrcFileCache {
    ring_prefix: Vec<usize>,
    span_index: HashMap<LocalId, (u32, u32)>,
}

/// Temporary storage for the parser to collect geometries.
#[derive(Default)]
pub(crate) struct GeometryCollector {
    pub vertices: indexmap::IndexSet<[u64; 3], ahash::RandomState>,
    pub geometry_crs_uri: Option<String>,
    pub multipolygon: MultiPolygon<'static, u32>,
    pub multilinestring: MultiLineString<'static, u32>,
    pub multipoint: MultiPoint<'static, u32>,

    /// ring ids of the all polygons
    pub ring_ids: Vec<Option<LocalId>>,

    /// surface polygon spans in `multipolygon`
    pub surface_spans: Vec<SurfaceSpan>,

    /// xlink:href references collected during geometry parsing (optional cross-file URL, fragment ID, flip winding)
    pub(crate) pending_hrefs: Vec<(Option<Url>, LocalId, bool)>,
}

impl GeometryCollector {
    pub fn add_exterior_ring(
        &mut self,
        iter: impl IntoIterator<Item = [f64; 3]>,
        ring_id: Option<LocalId>,
    ) {
        self.ring_ids.push(ring_id);
        self.multipolygon.add_exterior(iter.into_iter().map(|v| {
            let vbits = [v[0].to_bits(), v[1].to_bits(), v[2].to_bits()];
            let (index, _) = self.vertices.insert_full(vbits);
            index as u32
        }));
    }

    pub fn add_interior_ring(
        &mut self,
        iter: impl IntoIterator<Item = [f64; 3]>,
        ring_id: Option<LocalId>,
    ) {
        self.ring_ids.push(ring_id);
        self.multipolygon.add_interior(iter.into_iter().map(|v| {
            let vbits = [v[0].to_bits(), v[1].to_bits(), v[2].to_bits()];
            let (index, _) = self.vertices.insert_full(vbits);
            index as u32
        }));
    }

    pub fn add_linestring(&mut self, iter: impl IntoIterator<Item = [f64; 3]>) {
        self.multilinestring
            .add_linestring(iter.into_iter().map(|v| {
                let vbits = [v[0].to_bits(), v[1].to_bits(), v[2].to_bits()];
                let (index, _) = self.vertices.insert_full(vbits);
                index as u32
            }));
    }

    pub fn add_point(&mut self, point: [f64; 3]) {
        let vbits = [point[0].to_bits(), point[1].to_bits(), point[2].to_bits()];
        let (index, _) = self.vertices.insert_full(vbits);
        self.multipoint.push(index as u32);
    }

    pub fn into_geometries(self, envelope_crs_uri: Option<String>) -> GeometryStore {
        let mut vertices = Vec::with_capacity(self.vertices.len());
        for vbits in &self.vertices {
            vertices.push([
                f64::from_bits(vbits[0]),
                f64::from_bits(vbits[1]),
                f64::from_bits(vbits[2]),
            ]);
        }

        let crs_uri = envelope_crs_uri.unwrap_or(self.geometry_crs_uri.unwrap_or_default());

        let epsg = if crs_uri.starts_with(CRS_URI_EPSG_PREFIX) {
            if let Some(stripped) = crs_uri.strip_prefix(CRS_URI_EPSG_PREFIX) {
                stripped.parse::<EpsgCode>().ok()
            } else {
                None
            }
        } else {
            None
        }
        .unwrap_or(EPSG_JGD2011_GEOGRAPHIC_3D);

        GeometryStore {
            epsg,
            vertices,
            multipolygon: self.multipolygon,
            multilinestring: self.multilinestring,
            multipoint: self.multipoint,
            ring_ids: self.ring_ids,
            surface_spans: self.surface_spans,
            ..Default::default()
        }
    }
}
