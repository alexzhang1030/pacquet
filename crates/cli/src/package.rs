use crate::package_import::ImportMethodImpl;
use crate::package_manager::PackageManagerError;
use pacquet_npmrc::Npmrc;
use pacquet_registry::{Package, PackageVersion};
use pacquet_tarball::download_tarball_to_store;
use std::path::PathBuf;

/// This function execute the following and returns the package
/// - retrieves the package from the registry
/// - extracts the tarball to global store directory (~/Library/../pacquet)
/// - links global store directory to virtual dir (node_modules/.pacquet/..)
///
/// symlink_path will be appended by the name of the package. Therefore,
/// it should be resolved into the node_modules folder of a subdependency such as
/// `node_modules/.pacquet/fastify@1.0.0/node_modules`.
pub async fn find_package_version_from_registry<P: Into<PathBuf>>(
    config: &Npmrc,
    http_client: &reqwest::Client,
    name: &str,
    version: &str,
    symlink_path: P,
) -> Result<PackageVersion, PackageManagerError> {
    let package = Package::fetch_from_registry(name, http_client, &config.registry).await?;
    let package_version = package.get_pinned_version(version)?.unwrap();
    internal_fetch(package_version, config, symlink_path).await?;
    Ok(package_version.to_owned())
}

pub async fn fetch_package_version_directly<P: Into<PathBuf>>(
    config: &Npmrc,
    http_client: &reqwest::Client,
    name: &str,
    version: &str,
    symlink_path: P,
) -> Result<PackageVersion, PackageManagerError> {
    let package_version =
        PackageVersion::fetch_from_registry(name, version, http_client, &config.registry).await?;
    internal_fetch(&package_version, config, symlink_path).await?;
    Ok(package_version.to_owned())
}

async fn internal_fetch<P: Into<PathBuf>>(
    package_version: &PackageVersion,
    config: &Npmrc,
    symlink_path: P,
) -> Result<(), PackageManagerError> {
    let store_folder_name = package_version.get_store_name();
    let node_modules_path = config.virtual_store_dir.join(store_folder_name).join("node_modules");

    let cas_paths = download_tarball_to_store(
        &config.store_dir,
        &package_version.dist.integrity,
        package_version.dist.unpacked_size,
        package_version.get_tarball_url(),
    )
    .await?;

    config.package_import_method.import(
        &cas_paths,
        node_modules_path.join(&package_version.name),
        symlink_path.into().join(&package_version.name),
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::package::find_package_version_from_registry;
    use node_semver::Version;
    use pacquet_npmrc::Npmrc;
    use std::fs;
    use std::path::Path;
    use tempfile::tempdir;

    fn get_config(store_dir: &Path, modules_dir: &Path, virtual_store_dir: &Path) -> Npmrc {
        Npmrc {
            hoist: false,
            hoist_pattern: vec![],
            public_hoist_pattern: vec![],
            shamefully_hoist: false,
            store_dir: store_dir.to_path_buf(),
            modules_dir: modules_dir.to_path_buf(),
            node_linker: Default::default(),
            symlink: false,
            virtual_store_dir: virtual_store_dir.to_path_buf(),
            package_import_method: Default::default(),
            modules_cache_max_age: 0,
            lockfile: false,
            prefer_frozen_lockfile: false,
            lockfile_include_tarball_url: false,
            registry: "https://registry.npmjs.com/".to_string(),
            auto_install_peers: false,
            dedupe_peer_dependents: false,
            strict_peer_dependencies: false,
            resolve_peers_from_workspace_root: false,
        }
    }

    #[tokio::test]
    pub async fn should_find_package_version_from_registry() {
        let store_dir = tempdir().unwrap();
        let modules_dir = tempdir().unwrap();
        let virtual_store_dir = tempdir().unwrap();
        let config = get_config(store_dir.path(), modules_dir.path(), virtual_store_dir.path());
        let http_client = reqwest::Client::new();
        let symlink_path = tempdir().unwrap();
        let package = find_package_version_from_registry(
            &config,
            &http_client,
            "fast-querystring",
            "1.0.0",
            symlink_path.path(),
        )
        .await
        .unwrap();

        assert_eq!(package.name, "fast-querystring");
        assert_eq!(
            package.version,
            Version { major: 1, minor: 0, patch: 0, build: vec![], pre_release: vec![] }
        );

        let virtual_store_path = virtual_store_dir
            .path()
            .join(package.get_store_name())
            .join("node_modules")
            .join(&package.name);
        assert!(virtual_store_path.is_dir());

        // Make sure the symlink is resolving to the correct path
        assert_eq!(
            fs::read_link(symlink_path.path().join(&package.name)).unwrap(),
            virtual_store_path
        );
    }
}
