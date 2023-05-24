use rush_pdk::*;

// rush_plugin_init runs on startup
#[plugin_fn]
pub fn rush_plugin_init(input: Json<InitHookParams>) -> FnResult<()> {
    // print welcome message to console
    let text = format!(
        "Hello Rush {}! This is being run from a plugin.",
        input.0.rush_version
    );
    output_text(&text);

    // plugins can fetch single environment variables from the host...
    output_text(&format!("Your $LANG var: {:?}", env::get("LANG").0));

    // ...or all of them at once
    env::load_host_vars();
    output_text(&format!(
        "You have {} environment variables.",
        std::env::vars_os().count()
    ));

    Ok(())
}
