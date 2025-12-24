use std::path::{Path, PathBuf};

use hashbrown::HashMap;
use nusamai_citygml::{codelist::CodeResolver, ParseError};
use stretto::Cache;
use url::Url;

use super::xml::{parse_dictionary, Definition};

/// Internal error type to distinguish file-not-found from other errors
enum ResolveError {
    UrlJoinFailed,
    NotFilePath,
    FileNotFound,
    IoError(std::io::Error),
    ParseError(ParseError),
}

pub struct Resolver {
    cache: Cache<PathBuf, HashMap<String, Definition>>,
    fallback_base_urls: Vec<Url>,
}

impl Resolver {
    /// Create a new resolver with no fallback paths.
    pub fn new() -> Self {
        Self {
            cache: Cache::new(12960, 100000).unwrap(),
            fallback_base_urls: Vec::new(),
        }
    }

    /// Create a new resolver with fallback base URLs.
    ///
    /// When a codelist file is not found at the primary location (base_url + code_space),
    /// the resolver will try each fallback URL in order, joining it with just the
    /// filename portion of code_space.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let fallback = Url::parse("file:///common/codelists/").unwrap();
    /// let resolver = Resolver::with_fallback(vec![fallback]);
    ///
    /// // If CityGML at file:///data/city.gml references "../codelists/Building_usage.xml"
    /// // and file:///data/../codelists/Building_usage.xml doesn't exist,
    /// // it will try file:///common/codelists/Building_usage.xml
    /// ```
    pub fn with_fallback(fallback_base_urls: Vec<Url>) -> Self {
        // Normalize URLs to ensure they end with '/' for correct join behavior
        let normalized_urls = fallback_base_urls
            .into_iter()
            .map(|url| {
                let s = url.as_str();
                if s.ends_with('/') {
                    url
                } else {
                    Url::parse(&format!("{}/", s)).unwrap_or(url)
                }
            })
            .collect();

        Self {
            cache: Cache::new(12960, 100000).unwrap(),
            fallback_base_urls: normalized_urls,
        }
    }

    /// Internal helper to resolve from a specific URL.
    /// Returns Ok(Some(value)) on success, Ok(None) if code not found in dict,
    /// Err for various failure modes.
    fn try_resolve_from_url(
        &self,
        base_url: &Url,
        code_space: &str,
        code: &str,
    ) -> Result<Option<String>, ResolveError> {
        let abs_url = base_url
            .join(code_space)
            .map_err(|_| ResolveError::UrlJoinFailed)?;

        let path = abs_url
            .to_file_path()
            .map_err(|_| ResolveError::NotFilePath)?;

        // Check cache first
        if let Some(dict) = self.cache.get(&path) {
            return Ok(dict.value().get(code).map(|d| d.value().to_string()));
        }

        // Try to open file
        let file = match std::fs::File::open(&path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(ResolveError::FileNotFound);
            }
            Err(e) => {
                return Err(ResolveError::IoError(e));
            }
        };

        let reader = std::io::BufReader::with_capacity(128 * 1024, file);
        let definitions = parse_dictionary(reader).map_err(ResolveError::ParseError)?;

        let v = definitions.get(code).map(|d| d.value().to_string());
        let cost = definitions.len() as i64;
        self.cache.insert(path, definitions, cost);
        self.cache.wait().unwrap();
        Ok(v)
    }
}

impl Default for Resolver {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Resolver {
    fn drop(&mut self) {
        self.cache.close().unwrap();
    }
}

impl CodeResolver for Resolver {
    fn resolve(
        &self,
        base_url: &Url,
        code_space: &str,
        code: &str,
    ) -> Result<Option<String>, nusamai_citygml::ParseError> {
        // Try primary path first
        match self.try_resolve_from_url(base_url, code_space, code) {
            Ok(result) => return Ok(result),
            Err(ResolveError::FileNotFound) => {
                // Continue to fallback
            }
            Err(ResolveError::UrlJoinFailed) => {
                return Err(ParseError::CodelistError(format!(
                    "failed to join url: {base_url:?} + {code_space:?}"
                )));
            }
            Err(ResolveError::NotFilePath) => {
                return Err(ParseError::CodelistError(format!(
                    "failed to convert url to file path: {base_url:?} + {code_space:?}"
                )));
            }
            Err(ResolveError::IoError(e)) => {
                return Err(ParseError::CodelistError(format!(
                    "IO error reading codelist: {e}"
                )));
            }
            Err(ResolveError::ParseError(e)) => {
                return Err(e);
            }
        }

        // Primary path file not found - try fallbacks
        if self.fallback_base_urls.is_empty() {
            let abs_url = base_url.join(code_space).ok();
            return Err(ParseError::CodelistError(format!(
                "codelist file not found: {:?}",
                abs_url
            )));
        }

        // Extract filename from code_space for fallback lookup
        let filename = Path::new(code_space)
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or(code_space);

        let mut tried_paths = vec![base_url.join(code_space).ok()];

        for fallback_url in &self.fallback_base_urls {
            match self.try_resolve_from_url(fallback_url, filename, code) {
                Ok(result) => return Ok(result),
                Err(ResolveError::FileNotFound) => {
                    tried_paths.push(fallback_url.join(filename).ok());
                    continue;
                }
                Err(ResolveError::ParseError(e)) => return Err(e),
                Err(_) => continue,
            }
        }

        // All paths exhausted
        Err(ParseError::CodelistError(format!(
            "codelist file not found in any location. code_space: {}, tried: {:?}",
            code_space,
            tried_paths.into_iter().flatten().collect::<Vec<_>>()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nusamai_citygml::codelist::CodeResolver;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_codelist(dir: &Path, filename: &str, code: &str, description: &str) {
        let content = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<gml:Dictionary xmlns:gml="http://www.opengis.net/gml" gml:id="test">
    <gml:name>Test</gml:name>
    <gml:dictionaryEntry>
        <gml:Definition gml:id="id1">
            <gml:description>{}</gml:description>
            <gml:name>{}</gml:name>
        </gml:Definition>
    </gml:dictionaryEntry>
</gml:Dictionary>"#,
            description, code
        );
        fs::write(dir.join(filename), content).unwrap();
    }

    #[test]
    fn test_resolve_primary_path() {
        let temp = TempDir::new().unwrap();
        let codelists_dir = temp.path().join("codelists");
        fs::create_dir(&codelists_dir).unwrap();
        create_test_codelist(&codelists_dir, "Building_usage.xml", "401", "業務施設");

        let resolver = Resolver::new();
        let base_url = Url::from_file_path(temp.path().join("dummy.gml")).unwrap();

        let result = resolver
            .resolve(&base_url, "codelists/Building_usage.xml", "401")
            .unwrap();
        assert_eq!(result, Some("業務施設".to_string()));
    }

    #[test]
    fn test_resolve_code_not_in_dict() {
        let temp = TempDir::new().unwrap();
        let codelists_dir = temp.path().join("codelists");
        fs::create_dir(&codelists_dir).unwrap();
        create_test_codelist(&codelists_dir, "Building_usage.xml", "401", "業務施設");

        let resolver = Resolver::new();
        let base_url = Url::from_file_path(temp.path().join("dummy.gml")).unwrap();

        let result = resolver
            .resolve(&base_url, "codelists/Building_usage.xml", "999")
            .unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_resolve_file_not_found_no_fallback() {
        let temp = TempDir::new().unwrap();

        let resolver = Resolver::new();
        let base_url = Url::from_file_path(temp.path().join("dummy.gml")).unwrap();

        let result = resolver.resolve(&base_url, "codelists/Missing.xml", "401");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ParseError::CodelistError(_)));
    }

    #[test]
    fn test_resolve_fallback_success() {
        let temp = TempDir::new().unwrap();

        // Primary path: temp/primary/ (no codelist here)
        let primary_dir = temp.path().join("primary");
        fs::create_dir(&primary_dir).unwrap();

        // Fallback path: temp/fallback/codelists/ (codelist here)
        let fallback_dir = temp.path().join("fallback");
        fs::create_dir(&fallback_dir).unwrap();
        create_test_codelist(&fallback_dir, "Building_usage.xml", "401", "Fallback値");

        let fallback_url = Url::from_file_path(&fallback_dir).unwrap();
        let resolver = Resolver::with_fallback(vec![fallback_url]);

        let base_url = Url::from_file_path(primary_dir.join("dummy.gml")).unwrap();

        // Reference uses relative path that doesn't exist in primary
        let result = resolver
            .resolve(&base_url, "../codelists/Building_usage.xml", "401")
            .unwrap();
        assert_eq!(result, Some("Fallback値".to_string()));
    }

    #[test]
    fn test_fallback_url_normalization_no_trailing_slash() {
        let temp = TempDir::new().unwrap();

        let primary_dir = temp.path().join("primary");
        fs::create_dir(&primary_dir).unwrap();

        let fallback_dir = temp.path().join("fallback");
        fs::create_dir(&fallback_dir).unwrap();
        create_test_codelist(&fallback_dir, "Test.xml", "001", "Normalized");

        // Create URL WITHOUT trailing slash - should be normalized internally
        let fallback_url_str = format!("file://{}", fallback_dir.display());
        let fallback_url = Url::parse(&fallback_url_str).unwrap();
        assert!(!fallback_url.as_str().ends_with('/'));

        let resolver = Resolver::with_fallback(vec![fallback_url]);

        let base_url = Url::from_file_path(primary_dir.join("dummy.gml")).unwrap();

        let result = resolver
            .resolve(&base_url, "../nonexistent/Test.xml", "001")
            .unwrap();
        assert_eq!(result, Some("Normalized".to_string()));
    }

    #[test]
    fn test_primary_takes_precedence_over_fallback() {
        let temp = TempDir::new().unwrap();

        // Primary path with codelist
        let primary_codelists = temp.path().join("primary").join("codelists");
        fs::create_dir_all(&primary_codelists).unwrap();
        create_test_codelist(&primary_codelists, "Test.xml", "001", "Primary値");

        // Fallback path with same codelist but different value
        let fallback_dir = temp.path().join("fallback");
        fs::create_dir(&fallback_dir).unwrap();
        create_test_codelist(&fallback_dir, "Test.xml", "001", "Fallback値");

        let fallback_url = Url::from_file_path(&fallback_dir).unwrap();
        let resolver = Resolver::with_fallback(vec![fallback_url]);

        let base_url = Url::from_file_path(temp.path().join("primary").join("dummy.gml")).unwrap();

        let result = resolver
            .resolve(&base_url, "codelists/Test.xml", "001")
            .unwrap();
        assert_eq!(result, Some("Primary値".to_string()));
    }

    #[test]
    fn test_multiple_fallbacks_tried_in_order() {
        let temp = TempDir::new().unwrap();

        let primary_dir = temp.path().join("primary");
        fs::create_dir(&primary_dir).unwrap();

        // Fallback 1: empty
        let fallback1_dir = temp.path().join("fallback1");
        fs::create_dir(&fallback1_dir).unwrap();

        // Fallback 2: has the codelist
        let fallback2_dir = temp.path().join("fallback2");
        fs::create_dir(&fallback2_dir).unwrap();
        create_test_codelist(&fallback2_dir, "Test.xml", "001", "FromFallback2");

        let resolver = Resolver::with_fallback(vec![
            Url::from_file_path(&fallback1_dir).unwrap(),
            Url::from_file_path(&fallback2_dir).unwrap(),
        ]);

        let base_url = Url::from_file_path(primary_dir.join("dummy.gml")).unwrap();

        let result = resolver
            .resolve(&base_url, "../missing/Test.xml", "001")
            .unwrap();
        assert_eq!(result, Some("FromFallback2".to_string()));
    }

    #[test]
    fn test_all_fallbacks_exhausted() {
        let temp = TempDir::new().unwrap();

        let primary_dir = temp.path().join("primary");
        fs::create_dir(&primary_dir).unwrap();

        let fallback1_dir = temp.path().join("fallback1");
        fs::create_dir(&fallback1_dir).unwrap();

        let fallback2_dir = temp.path().join("fallback2");
        fs::create_dir(&fallback2_dir).unwrap();

        let resolver = Resolver::with_fallback(vec![
            Url::from_file_path(&fallback1_dir).unwrap(),
            Url::from_file_path(&fallback2_dir).unwrap(),
        ]);

        let base_url = Url::from_file_path(primary_dir.join("dummy.gml")).unwrap();

        let result = resolver.resolve(&base_url, "../missing/Missing.xml", "001");
        assert!(result.is_err());
        let err_msg = format!("{:?}", result.unwrap_err());
        assert!(err_msg.contains("codelist file not found in any location"));
    }
}
