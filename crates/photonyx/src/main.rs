use siril_sys::Siril;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Hello from photonyx!");

    let file = "'/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/NGC 2244/2026-02-02/RAW_O_300.00s/2026-02-02_22-49-25_O_-9.90c_100g_30o_300.00s_d_1x1_0720.fits'";

    // Startup and wait till process is ready for additional commands
    let mut siril = Siril::new().await?;
    siril.command("requires 0.99.10").await?;
    siril.command("set core.mem_ratio=0.9").await?;
    siril.command(&format!("load {}", file)).await?;

    let stat_output = siril.command("stat").await;
    for line in &stat_output.unwrap() {
        println!("stat: {:?}", line);
    }

    siril.close().await?;
    // Siril also cleans up when dropped

    Ok(())
}
