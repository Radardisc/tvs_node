

use tfs_http::tfs_web_server_builder::{TfsWebServerBuilder, TfsWebServerRunner};
use tvs::{
    services::tfs_services_adapter::ActualTfsAppInterfaceAdapter,
    webserver::{TvsWebServer, TvsWebServerRunner, TvsConfig},
};

use crate::config::TvsNodeConfig;

#[cfg(feature = "postgres")]
use tvs_postgres::{PostgresVoteService, PostgresVoteUrlService, initialize_tvs_tables};
#[cfg(feature = "postgres")]
use tfs_postgres::{establish_connection_pool, DbSession, SchemaContext, DbPool};

pub struct TvsNodeRunner {
    tfs_web_server_runner: TfsWebServerRunner,
    tvs_web_server_runner: Option<TvsWebServerRunner>
}

impl TvsNodeRunner {
    pub async fn build_with_config(
        config: TvsNodeConfig,
    ) -> Result<TvsNodeRunner, Box<dyn std::error::Error>> {
        // Configure admin frontend based on feature flag
        let mut tfs_config = config.tfs.clone();
        Self::configure_admin_frontend(&mut tfs_config)?;

        let mut tfs_web_server_builder = TfsWebServerBuilder::new(tfs_config);

        tfs_web_server_builder
            .setup_node()
            .init_tracing()
            .setup_app_interface()
            .setup_app_shell();

        // Start TFS web server
        let tfs_web_server_runner = tfs_web_server_builder
            .start_webserver()
            .await?;

        let node_service = tfs_web_server_runner.webserver().shell.app.get_this_node_id();

        // Configure TVS services after server is running
        let app_interface = tfs_web_server_runner.webserver().shell.app.clone();
        Self::configure_tvs_services(&node_service, app_interface.clone())?;

        // Optionally start TVS vote server on separate port
        let tvs_runner = Self::start_tvs_vote_server(&node_service, app_interface, config.tvs).await?;

        Ok(Self {
            tfs_web_server_runner,
            tvs_web_server_runner: tvs_runner
        })
    }

    /// Configure TVS services (VoteService and VoteUrlService) based on enabled features
    fn configure_tvs_services(
        node_id: &tfs::tfs::node_id::NodeId,
        app_interface: tfs::tfs_app_interface::TFSAppInterface,
    ) -> Result<(), Box<dyn std::error::Error>> {
        #[cfg(feature = "postgres")]
        {
            // Establish shared connection pool for both TFS and TVS
            let db_pool = establish_connection_pool();
            let schema_ctx = SchemaContext::from_node_id(node_id, false);
            let session = DbSession::new(db_pool.clone(), schema_ctx);

            // Initialize schema and run migrations
            session.initialize_schema()?;
            initialize_tvs_tables(&session)?;

            // Configure PostgreSQL-backed vote service
            let vote_service = PostgresVoteService::new(session.clone());
            tvs::services::vote_service::configure_vote_service(
                node_id,
                Box::new(vote_service),
            )?;

            // Configure PostgreSQL-backed vote URL service
            let root_url = std::env::var("TVS_ROOT_URL")
                .unwrap_or_else(|_| "http://localhost:8081/vote".to_string());
            let vote_url_service = PostgresVoteUrlService::with_root_url(session, root_url);
            tvs::services::vote_url_service::configure_vote_url_service(
                node_id,
                Box::new(vote_url_service),
            )?;

            println!("✓ Configured PostgreSQL persistence for node: {}", node_id);
        }

        #[cfg(all(feature = "ephemeral", not(feature = "postgres")))]
        {
            // Create TFS adapter for ephemeral vote service
            let tfs_adapter = ActualTfsAppInterfaceAdapter::as_tfs_app_interface_adapter(
                node_id,
                app_interface,
            );

            // Configure ephemeral (in-memory) vote service
            tvs::services::vote_service::configure_ephemeral_vote_service(node_id, tfs_adapter)?;

            // Configure ephemeral (in-memory) vote URL service
            let root_url = std::env::var("TVS_ROOT_URL")
                .unwrap_or_else(|_| "http://localhost:8081/vote".to_string());
            tvs::services::vote_url_service::configure_ephemeral_vote_url_service(
                node_id,
                root_url,
            )?;

            println!("✓ Configured ephemeral (in-memory) persistence for node: {}", node_id);
        }

        Ok(())
    }

    /// Configure admin frontend availability based on feature flag
    fn configure_admin_frontend(_config: &mut tfs_http::app_config::AppConfig) -> Result<(), Box<dyn std::error::Error>> {
        #[cfg(feature = "admin-frontend")]
        {
            println!("✓ Admin frontend enabled");
            println!("  Admin UI: http://localhost:{}/static/private", _config.server.admin_port);
            println!("  Admin API: http://localhost:{}/tfs/admin", _config.server.admin_port);
        }

        #[cfg(not(feature = "admin-frontend"))]
        {
            println!("⚠ Admin frontend disabled (no separate admin port)");
            println!("  Admin routes consolidated with cluster port");
        }

        Ok(())
    }

    /// Start TVS vote server if vote service is configured
    async fn start_tvs_vote_server(
        node_id: &tfs::tfs::node_id::NodeId,
        app_interface: tfs::tfs_app_interface::TFSAppInterface,
        tvs_server_config: Option<crate::config::TvsServerConfig>,
    ) -> Result<Option<TvsWebServerRunner>, Box<dyn std::error::Error>> {
        // Check if vote service is configured for this node
        if let Some(vote_service) = tvs::services::vote_service::get_vote_service(node_id) {
            // Get TVS config from config file, with environment variable overrides
            let tvs_config = if let Some(config) = tvs_server_config {
                if !config.enabled {
                    println!("⚠ TVS vote server disabled in configuration");
                    return Ok(None);
                }

                TvsConfig {
                    vote_service_port: std::env::var("TVS_VOTE_PORT")
                        .ok()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(config.vote_port),
                    vote_service_host: std::env::var("TVS_VOTE_HOST")
                        .unwrap_or(config.vote_host),
                }
            } else {
                // No config section - use environment or defaults
                TvsConfig {
                    vote_service_port: std::env::var("TVS_VOTE_PORT")
                        .ok()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(8090),
                    vote_service_host: std::env::var("TVS_VOTE_HOST")
                        .unwrap_or_else(|_| "127.0.0.1".to_string()),
                }
            };

            println!("✓ Starting TVS vote server on {}:{}",
                tvs_config.vote_service_host, tvs_config.vote_service_port);

            let tvs_runner = TvsWebServer::start_vote_server(
                vote_service,
                app_interface,
                tvs_config,
            ).await?;

            Ok(Some(tvs_runner))
        } else {
            println!("⚠ No vote service configured - TVS vote server disabled");
            println!("  Vote routes will not be available");
            Ok(None)
        }
    }

    pub async fn run_until_shutdown(mut self) -> Result<(), Box<dyn std::error::Error>> {
        // If TVS vote server is running, run both servers concurrently
        if let Some(tvs_runner) = self.tvs_web_server_runner {
            println!("Running both TFS and TVS servers until shutdown...");
            TvsWebServer::run_both_until_shutdown(
                self.tfs_web_server_runner,
                tvs_runner,
            ).await;
            Ok(())
        } else {
            // Just run TFS server
            self.tfs_web_server_runner.run_until_shutdown().await
        }
    }

}