use arcweft_launch::ResolvedLaunchProfile;

fn reject_policy_accessor(profile: &ResolvedLaunchProfile) {
    let _ = profile.capability_policy();
}

fn main() {}
