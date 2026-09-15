use japan_geoid::{gsi::MemoryGrid, Geoid};

/// Vertical transformation applied to geographic coordinates.
///
/// Implementations convert a `(lng, lat, height)` triple and return the
/// transformed triple. Longitude and latitude are passed through unchanged by
/// every implementation in this module; only the height component changes.
pub trait VerticalTransform: Send + Sync {
    fn convert(&self, lng: f64, lat: f64, height: f64) -> (f64, f64, f64);
}

#[derive(Debug)]
/// Convert from JGD 2011 Geograhpic 3D (EPSG:6697) to WGS84 Geograhpic 3D (EPSG:4979)
pub struct Jgd2011ToWgs84 {
    geoid: MemoryGrid<'static>,
}

impl Jgd2011ToWgs84 {
    /// Create a new instance with the embed geoid model data.
    pub fn new() -> Self {
        Self {
            geoid: japan_geoid::gsi::load_embedded_gsigeo2011(),
        }
    }

    /// JGD2011 Geographic 3D (EPSG:6697) to WGS84 Geographic 3D (EPSG:4979)
    pub fn convert(&self, lng: f64, lat: f64, height: f64) -> (f64, f64, f64) {
        let ellipsoid_height = self.geoid.get_height(lng, lat) + height;
        (lng, lat, ellipsoid_height)
    }
}

impl Default for Jgd2011ToWgs84 {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for Jgd2011ToWgs84 {
    fn clone(&self) -> Self {
        Self::new()
    }
}

impl VerticalTransform for Jgd2011ToWgs84 {
    fn convert(&self, lng: f64, lat: f64, height: f64) -> (f64, f64, f64) {
        Jgd2011ToWgs84::convert(self, lng, lat, height)
    }
}

#[derive(Debug)]
/// Convert from JGD2024 Geographic 3D (EPSG:6668 + EPSG:11317) to WGS84
/// Geographic 3D (EPSG:4979).
///
/// Uses the JPGEO2024 geoid model combined with the Hrefconv2024 reference
/// surface correction, so heights on remote islands that refer to a local mean
/// sea level are handled as well as mainland heights.
pub struct Jgd2024ToWgs84 {
    geoid: MemoryGrid<'static>,
}

impl Jgd2024ToWgs84 {
    /// Create a new instance with the embedded JPGEO2024 + Hrefconv2024 model.
    pub fn new() -> Self {
        Self {
            geoid: japan_geoid::gsi::load_embedded_jpgeo2024_hrefconv2024(),
        }
    }

    /// JGD2024 Geographic 3D to WGS84 Geographic 3D (EPSG:4979)
    pub fn convert(&self, lng: f64, lat: f64, height: f64) -> (f64, f64, f64) {
        let ellipsoid_height = self.geoid.get_height(lng, lat) + height;
        (lng, lat, ellipsoid_height)
    }
}

impl Default for Jgd2024ToWgs84 {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for Jgd2024ToWgs84 {
    fn clone(&self) -> Self {
        Self::new()
    }
}

impl VerticalTransform for Jgd2024ToWgs84 {
    fn convert(&self, lng: f64, lat: f64, height: f64) -> (f64, f64, f64) {
        Jgd2024ToWgs84::convert(self, lng, lat, height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixtures() {
        let (lng_jgd, lat_jgd, elevation) = (138.2839817085188, 37.12378643088312, 0.);
        let jgd_to_wgs = Jgd2011ToWgs84::new();
        let (lng_wgs, lat_wgs, ellips_height) = jgd_to_wgs.convert(lng_jgd, lat_jgd, elevation);
        assert!((ellips_height - 39.47387115961899).abs() < 1e-8);
        // (lng, lat) must not change.
        assert_eq!(lng_jgd, lng_wgs);
        assert_eq!(lat_jgd, lat_wgs);
    }

    #[test]
    fn jgd2024_fixtures() {
        // Tokyo, the fixture japan-geoid itself uses for the combined model.
        let (lng, lat, elevation) = (139.6917, 35.6895, 0.);
        let jgd_to_wgs = Jgd2024ToWgs84::new();
        let (lng_wgs, lat_wgs, ellips_height) = jgd_to_wgs.convert(lng, lat, elevation);
        assert!(
            (ellips_height - 37.0983).abs() < 0.01,
            "got {ellips_height}"
        );
        assert_eq!(lng, lng_wgs);
        assert_eq!(lat, lat_wgs);
    }

    #[test]
    fn trait_object() {
        let t: Box<dyn VerticalTransform> = Box::new(Jgd2024ToWgs84::new());
        let (_, _, h) = t.convert(139.6917, 35.6895, 10.0);
        assert!((h - 47.0983).abs() < 0.01);
    }
}
