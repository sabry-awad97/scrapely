use scrapely::*;

#[cfg(test)]
mod url_normalizer_tests {
    use super::*;

    #[test]
    fn test_normalize_removes_fragment() {
        let url = "https://example.com/page#section";
        let normalized = UrlNormalizer::normalize(url);
        assert_eq!(normalized, "https://example.com/page");
    }

    #[test]
    fn test_normalize_removes_trailing_slash_from_path() {
        let url = "https://example.com/page/";
        let normalized = UrlNormalizer::normalize(url);
        assert_eq!(normalized, "https://example.com/page");
    }

    #[test]
    fn test_normalize_keeps_trailing_slash_for_root() {
        let url = "https://example.com/";
        let normalized = UrlNormalizer::normalize(url);
        assert_eq!(normalized, "https://example.com/");
    }

    #[test]
    fn test_normalize_handles_query_parameters() {
        let url = "https://example.com/page?param=value";
        let normalized = UrlNormalizer::normalize(url);
        assert_eq!(normalized, "https://example.com/page?param=value");
    }

    #[test]
    fn test_normalize_query_params_with_trailing_slash() {
        // This test validates the fix: should not count slashes in query params
        let url = "https://example.com/page/?param=value/with/slashes";
        let normalized = UrlNormalizer::normalize(url);

        // Should remove trailing slash from path, not count slashes in query
        assert_eq!(
            normalized,
            "https://example.com/page?param=value/with/slashes"
        );
    }

    #[test]
    fn test_normalize_fragment_and_query() {
        let url = "https://example.com/page?param=value#section";
        let normalized = UrlNormalizer::normalize(url);
        assert_eq!(normalized, "https://example.com/page?param=value");
    }

    #[test]
    fn test_normalize_deep_path() {
        let url = "https://example.com/a/b/c/d/";
        let normalized = UrlNormalizer::normalize(url);
        assert_eq!(normalized, "https://example.com/a/b/c/d");
    }

    #[test]
    fn test_normalize_idempotent() {
        let url = "https://example.com/page";
        let normalized1 = UrlNormalizer::normalize(url);
        let normalized2 = UrlNormalizer::normalize(&normalized1);
        assert_eq!(normalized1, normalized2);
    }
}
