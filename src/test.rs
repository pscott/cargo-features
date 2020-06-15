#[cfg(test)]
mod tests {
    // we do not define a TEST_DIR because concat! does not work with consts...
    const NO_FEATURES_FILE: &str = "test_files/no_features.rs";
    const ONE_FEATURE_FILE: &str = "test_files/one_feature.rs";
    const FOUR_FEATURES_FILE: &str = "test_files/four_features.rs";
    const ONE_LINE_FEATURES_FILE: &str = "test_files/one_line_features.rs";
    use crate::package::Package;
    use std::collections::HashSet;
    use std::path::{Path, PathBuf};

    fn find_and_check(package: &mut Package, path: &Path) -> Result<(), String> {
        package.find_used_features(path)?;
        package.find_exposed_features();
        package.find_hidden_features();
        package.check_hidden_features()
    }

    #[test]
    fn empty_features() {
        let excluded_paths = HashSet::new();
        let excluded_features = HashSet::new();
        let p = Package::new(excluded_paths, excluded_features);
        let res = p.check_hidden_features();
        dbg!(&res);
        assert!(res.is_ok());
    }

    #[test]
    fn no_features() {
        let excluded_paths = HashSet::new();
        let excluded_features = HashSet::new();
        let mut p = Package::new(excluded_paths, excluded_features);
        let path = PathBuf::from(NO_FEATURES_FILE);
        let res = find_and_check(&mut p, &path);
        dbg!(&res);
        assert!(res.is_ok());
    }

    #[test]
    fn does_not_exist() {
        let excluded_paths = HashSet::new();
        let excluded_features = HashSet::new();
        let mut p = Package::new(excluded_paths, excluded_features);
        let path = PathBuf::new();
        let res = find_and_check(&mut p, &path);
        dbg!(&res);
        assert!(res.is_err());
    }

    #[test]
    fn one_feature() {
        let excluded_paths = HashSet::new();
        let excluded_features = HashSet::new();
        let mut p = Package::new(excluded_paths, excluded_features);
        let path = PathBuf::from(ONE_FEATURE_FILE);
        let res = find_and_check(&mut p, &path);
        let features = p.hidden_features();
        assert!(features.contains("hidden-feature"));
        dbg!(&res);
        assert!(res.is_err());
    }

    #[test]
    fn one_feature_but_excluded() {
        let excluded_paths = HashSet::new();
        let mut excluded_features = HashSet::new();
        excluded_features.insert(String::from("hidden-feature"));
        let mut p = Package::new(excluded_paths, excluded_features);
        let path = PathBuf::from(ONE_FEATURE_FILE);
        let res = find_and_check(&mut p, &path);
        let features = p.hidden_features();
        assert!(features.is_empty());
        dbg!(&res);
        assert!(res.is_ok());
    }

    #[test]
    fn one_feature_but_path_excluded() {
        let mut excluded_paths = HashSet::new();
        excluded_paths.insert(PathBuf::from(ONE_FEATURE_FILE));
        let excluded_features = HashSet::new();
        let mut p = Package::new(excluded_paths, excluded_features);
        let path = PathBuf::from(ONE_FEATURE_FILE);
        let res = find_and_check(&mut p, &path);
        let features = p.hidden_features();
        assert!(features.is_empty());
        dbg!(&res);
        assert!(res.is_ok());
    }

    #[test]
    fn four_features() {
        let excluded_paths = HashSet::new();
        let excluded_features = HashSet::new();
        let mut p = Package::new(excluded_paths, excluded_features);
        let path = PathBuf::from(FOUR_FEATURES_FILE);
        let res = find_and_check(&mut p, &path);
        let mut features = p.hidden_features();
        dbg!(&features);
        assert!(features.remove("hidden-feature-1"));
        assert!(features.remove("hidden-feature-2"));
        assert!(features.remove("hidden-feature-3"));
        assert!(features.remove("hidden-feature-4"));
        assert!(features.is_empty());
        dbg!(&res);
        assert!(res.is_err());
    }

    #[test]
    fn one_line_features() {
        let excluded_paths = HashSet::new();
        let excluded_features = HashSet::new();
        let mut p = Package::new(excluded_paths, excluded_features);
        let path = PathBuf::from(ONE_LINE_FEATURES_FILE);
        let res = find_and_check(&mut p, &path);
        let mut features = p.hidden_features();
        dbg!(&features);
        assert!(features.remove("get-your"));
        assert!(features.remove("shit-together"));
        assert!(features.remove("get-it-all-together"));
        assert!(features.remove("and-put-it-all-in-a-backpack"));
        assert!(features.is_empty());
        dbg!(&res);
        assert!(res.is_err());
    }
}
