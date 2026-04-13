use api::{app::Application, config::Config, telemetry};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;
    telemetry::init(&config.observability)?;

    let app = Application::build(config).await?;
    app.run().await?;

    Ok(())
}
