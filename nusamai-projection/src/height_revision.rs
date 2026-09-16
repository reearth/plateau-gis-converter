//! Height revision parameters published by the Geospatial Information
//! Authority of Japan (GSI) for the move from 測地成果2011 to 測地成果2024.
//!
//! The 2025 nationwide revision of Japanese heights changed orthometric heights
//! by up to about 0.7 m. GSI publishes the revision as a grid of corrections at
//! the south-west corner node of every third-order standard mesh cell
//! (30 arcseconds by 45 arcseconds), in the PatchJGD(H) parameter file
//! `hyokorevTR_jgd2024_h.par`. A correction at an arbitrary position is
//! obtained by bilinear interpolation over the four surrounding nodes, which is
//! what the official PatchJGD software does.
//!
//! GSI publishes two parameter sets: `hyokorevBM_jgd2024_h.par` for heights
//! referenced to levelling benchmarks (水準点) and `hyokorevTR_jgd2024_h.par`
//! for heights referenced to triangulation points (三角点). Version 1.0.0 of
//! both is embedded in a compact lossless binary form; see
//! [`HeightRevisionGrid::load_embedded`].

use std::io::{self, Read};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::vshift::VerticalTransform;

/// Number of grid nodes per degree of latitude (30 arcsecond spacing).
const LAT_NODES_PER_DEG: f64 = 120.0;
/// Number of grid nodes per degree of longitude (45 arcsecond spacing).
const LON_NODES_PER_DEG: f64 = 80.0;
/// Longitude of grid column zero, as defined by the standard mesh code.
const LON_ORIGIN_DEG: f64 = 100.0;

/// Layout of the embedded file, after lz4 decompression: a magic, a format
/// version, the grid origin and size in global node units (30 arcsecond rows
/// from the equator, 45 arcsecond columns from 100°E), the version line of the
/// GSI file, then runs of consecutive nodes along a row. Each run is its row,
/// start column and length, followed by the corrections as 32 bit integers in
/// units of 0.01 mm, delta coded as zigzag varints.
const BINARY_MAGIC: &[u8; 4] = b"HREV";
const BINARY_FORMAT_VERSION: u8 = 1;
const BINARY_SCALE: f64 = 1e5;

/// Which of GSI's two parameter sets to use.
///
/// GSI's own base map update applied the benchmark set and fell back to the
/// triangulation set where the benchmark set has no coverage, which is why a
/// consumer typically holds both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterSet {
    /// `hyokorevBM_jgd2024_h.par`: built from the levelling network. Excludes
    /// some islands and the parts of Miyazaki and Ishikawa disturbed by
    /// earthquakes.
    Benchmark,
    /// `hyokorevTR_jgd2024_h.par`: built from the GNSS station ellipsoid
    /// height changes and the geoid model change. Excludes some islands.
    Triangulation,
}

/// Dense grid of height corrections with `NaN` marking nodes without data.
#[derive(Debug, Clone)]
pub struct HeightRevisionGrid {
    row0: u32,
    col0: u32,
    rows: usize,
    cols: usize,
    values: Vec<f32>,
    version: String,
}

impl HeightRevisionGrid {
    /// One of the embedded nationwide parameter sets.
    pub fn load_embedded(set: ParameterSet) -> Self {
        const BM: &[u8] = include_bytes!("hyokorevBM_jgd2024_h.bin.lz4");
        const TR: &[u8] = include_bytes!("hyokorevTR_jgd2024_h.bin.lz4");
        let embedded = match set {
            ParameterSet::Benchmark => BM,
            ParameterSet::Triangulation => TR,
        };
        let bytes = lz4_flex::decompress_size_prepended(embedded)
            .expect("embedded height revision grid is valid lz4");
        Self::from_binary_reader(&mut io::Cursor::new(bytes))
            .expect("embedded height revision grid is valid")
    }

    fn from_binary_reader<R: Read>(reader: &mut R) -> io::Result<Self> {
        let invalid = |msg: &str| io::Error::new(io::ErrorKind::InvalidData, msg.to_string());
        let mut head = [0u8; 5];
        reader.read_exact(&mut head)?;
        if &head[..4] != BINARY_MAGIC {
            return Err(invalid("not a height revision grid"));
        }
        if head[4] != BINARY_FORMAT_VERSION {
            return Err(invalid("unsupported height revision grid format version"));
        }
        let row0 = read_u32(reader)?;
        let col0 = read_u32(reader)?;
        let rows = read_u32(reader)? as usize;
        let cols = read_u32(reader)? as usize;
        let mut len = [0u8; 2];
        reader.read_exact(&mut len)?;
        let mut version = vec![0u8; u16::from_le_bytes(len) as usize];
        reader.read_exact(&mut version)?;
        let version = String::from_utf8(version).map_err(|_| invalid("version is not UTF-8"))?;
        let run_count = read_u32(reader)?;
        let count = rows
            .checked_mul(cols)
            .ok_or_else(|| invalid("grid too large"))?;
        let mut values = vec![f32::NAN; count];
        let mut rest = Vec::new();
        reader.read_to_end(&mut rest)?;
        let mut cursor = io::Cursor::new(rest);
        for _ in 0..run_count {
            let row = read_u32(&mut cursor)? as usize;
            let start = read_u32(&mut cursor)? as usize;
            let len = read_u32(&mut cursor)? as usize;
            if row >= rows || start + len > cols {
                return Err(invalid("run outside the grid"));
            }
            let mut previous = 0i32;
            for value in &mut values[row * cols + start..row * cols + start + len] {
                previous = previous.wrapping_add(unzigzag(read_varint(&mut cursor)?));
                *value = (previous as f64 / BINARY_SCALE) as f32;
            }
        }
        Ok(Self {
            row0,
            col0,
            rows,
            cols,
            values,
            version,
        })
    }

    /// First line of the parameter file, which carries the version.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Number of grid nodes with data.
    pub fn node_count(&self) -> usize {
        self.values.iter().filter(|v| !v.is_nan()).count()
    }

    /// Correction at a grid node, addressed in global node units.
    fn node(&self, row: i64, col: i64) -> Option<f64> {
        let row = row.checked_sub(self.row0 as i64)?;
        let col = col.checked_sub(self.col0 as i64)?;
        if row < 0 || col < 0 || row as usize >= self.rows || col as usize >= self.cols {
            return None;
        }
        let value = self.values[row as usize * self.cols + col as usize];
        if value.is_nan() {
            None
        } else {
            Some(value as f64)
        }
    }

    /// Bilinearly interpolated correction at a position, or `None` when any of
    /// the four surrounding nodes has no data.
    pub fn get(&self, lng: f64, lat: f64) -> Option<f64> {
        let x = (lng - LON_ORIGIN_DEG) * LON_NODES_PER_DEG;
        let y = lat * LAT_NODES_PER_DEG;
        if !(x.is_finite() && y.is_finite()) || x < 0.0 || y < 0.0 {
            return None;
        }
        // Snap positions that sit on a node up to floating point noise, so a
        // lookup exactly on a node never straddles into a neighbouring cell.
        let x = snap_to_node(x);
        let y = snap_to_node(y);
        let i = x.floor();
        let j = y.floor();
        let fx = x - i;
        let fy = y - j;
        let (i, j) = (i as i64, j as i64);
        let sw = self.node(j, i)?;
        let se = self.node(j, i + 1)?;
        let nw = self.node(j + 1, i)?;
        let ne = self.node(j + 1, i + 1)?;
        let south = sw + (se - sw) * fx;
        let north = nw + (ne - nw) * fx;
        Some(south + (north - south) * fy)
    }
}

fn read_u32<R: Read>(reader: &mut R) -> io::Result<u32> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

fn unzigzag(v: u32) -> i32 {
    ((v >> 1) as i32) ^ -((v & 1) as i32)
}

fn read_varint<R: Read>(reader: &mut R) -> io::Result<u32> {
    let mut result = 0u32;
    let mut shift = 0;
    loop {
        let mut byte = [0u8; 1];
        reader.read_exact(&mut byte)?;
        result |= ((byte[0] & 0x7f) as u32) << shift;
        if byte[0] & 0x80 == 0 {
            return Ok(result);
        }
        shift += 7;
        if shift > 28 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "varint too long",
            ));
        }
    }
}

fn snap_to_node(v: f64) -> f64 {
    let r = v.round();
    if (v - r).abs() < 1e-7 {
        r
    } else {
        v
    }
}

/// Converts JGD2011 orthometric heights (測地成果2011) to JGD2024 orthometric
/// heights (測地成果2024) by adding the GSI height revision correction.
///
/// Positions outside the grid coverage are passed through unchanged and the
/// miss flag is raised, so a caller can decide per feature whether to keep or
/// discard the result. Longitude and latitude are never changed.
#[derive(Debug)]
pub struct Jgd2011ToJgd2024 {
    grid: Arc<HeightRevisionGrid>,
    missed: AtomicBool,
}

impl Jgd2011ToJgd2024 {
    pub fn new(grid: Arc<HeightRevisionGrid>) -> Self {
        Self {
            grid,
            missed: AtomicBool::new(false),
        }
    }

    /// Returns whether any conversion since the last call fell outside the
    /// grid coverage, and clears the flag.
    pub fn take_missed(&self) -> bool {
        self.missed.swap(false, Ordering::Relaxed)
    }
}

impl Clone for Jgd2011ToJgd2024 {
    fn clone(&self) -> Self {
        Self::new(Arc::clone(&self.grid))
    }
}

impl VerticalTransform for Jgd2011ToJgd2024 {
    fn convert(&self, lng: f64, lat: f64, height: f64) -> (f64, f64, f64) {
        match self.grid.get(lng, lat) {
            Some(dh) => (lng, lat, height + dh),
            None => {
                self.missed.store(true, Ordering::Relaxed);
                (lng, lat, height)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Sendai station. The GSI file gives +0.1233 m here.
    const LNG: f64 = 140.8825;
    const LAT: f64 = 38.2592;

    #[test]
    fn embedded_grid() {
        let grid = HeightRevisionGrid::load_embedded(ParameterSet::Triangulation);
        assert!(grid.version().contains("Ver.1.0.0"), "{}", grid.version());
        assert_eq!(grid.node_count(), 645_301);
        let v = grid.get(LNG, LAT).unwrap();
        assert!((v - 0.1236).abs() < 5e-4, "got {v}");
        // Exactly on the south-west node of the cell containing the station.
        let x = ((LNG - LON_ORIGIN_DEG) * LON_NODES_PER_DEG).floor();
        let y = (LAT * LAT_NODES_PER_DEG).floor();
        let on_node = grid
            .get(
                LON_ORIGIN_DEG + x / LON_NODES_PER_DEG,
                y / LAT_NODES_PER_DEG,
            )
            .unwrap();
        assert_eq!(on_node, grid.node(y as i64, x as i64).unwrap());
        // Sea, well outside the coverage, and nonsense input.
        assert!(grid.get(135.0, 30.0).is_none());
        assert!(grid.get(f64::NAN, 30.0).is_none());
    }

    #[test]
    fn embedded_benchmark_grid() {
        let grid = HeightRevisionGrid::load_embedded(ParameterSet::Benchmark);
        assert!(grid.version().contains("Ver.1.0.0"), "{}", grid.version());
        assert_eq!(grid.node_count(), 643_467);
        // The benchmark set differs from the triangulation set by 14 cm here.
        let v = grid.get(LNG, LAT).unwrap();
        assert!((v + 0.0205).abs() < 5e-4, "got {v}");
        assert!(grid.get(135.0, 30.0).is_none());
    }

    #[test]
    fn varint_decoding() {
        // 0, 1, -1, 300 and -58341 as zigzag varints.
        let bytes = [0x00, 0x02, 0x01, 0xd8, 0x04, 0xc9, 0x8f, 0x07];
        let mut cursor = io::Cursor::new(bytes);
        let decoded: Vec<i32> = (0..5)
            .map(|_| unzigzag(read_varint(&mut cursor).unwrap()))
            .collect();
        assert_eq!(decoded, [0, 1, -1, 300, -58_341]);
        assert!(read_varint(&mut io::Cursor::new([0xff; 6])).is_err());
    }

    #[test]
    fn rejects_foreign_bytes() {
        assert!(HeightRevisionGrid::from_binary_reader(&mut io::Cursor::new(b"nope!")).is_err());
    }

    #[test]
    fn transform_and_miss_flag() {
        let t = Jgd2011ToJgd2024::new(Arc::new(HeightRevisionGrid::load_embedded(
            ParameterSet::Triangulation,
        )));
        let (lng, lat, h) = t.convert(LNG, LAT, 10.0);
        assert_eq!((lng, lat), (LNG, LAT));
        assert!((h - 10.1236).abs() < 5e-4, "got {h}");
        assert!(!t.take_missed());
        let (_, _, h) = t.convert(135.0, 30.0, 10.0);
        assert_eq!(h, 10.0);
        assert!(t.take_missed());
        assert!(!t.take_missed());
    }
}
