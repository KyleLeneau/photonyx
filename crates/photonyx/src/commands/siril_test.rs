// use std::path::Path;

use siril_sys::Builder;

pub async fn siril_test() -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("siril_test command called");

    // Startup and wait till process is ready for additional commands
    let _siril = Builder::default()
        // .use_directory(Path::new("/Users/kyle/Development/BortleSpace/photonyx"))
        .build()
        .await?;

    // let mut siril = Siril::new().await?;
    // siril.command("requires 0.99.10").await?;
    // siril.command("set core.mem_ratio=0.9").await?;
    // siril.command("set core.force_16bit=false").await?;
    // siril.command("get -a").await?;

    Ok(())
}
