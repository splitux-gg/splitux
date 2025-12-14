//! Profile setup operations

use crate::handler::Handler;
use crate::instance::Instance;
use crate::profiles::{create_profile, create_profile_gamesave};

/// Setup profiles for all instances
pub fn setup_profiles(
    h: &Handler,
    instances: &Vec<Instance>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n[splitux] Instances:");
    for instance in instances {
        if instance.profname.starts_with(".") {
            create_profile(&instance.profname)?;
        }
        if h.is_saved_handler() {
            create_profile_gamesave(&instance.profname, h)?;
        }
        println!(
            "[splitux] - Profile: {}, Monitor: {}, Resolution: {}x{}",
            instance.profname, instance.monitor, instance.width, instance.height
        );
    }

    Ok(())
}
