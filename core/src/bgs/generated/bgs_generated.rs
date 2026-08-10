pub mod bgs {
    pub mod protocol {
        include!("bgs.protocol.rs");
        pub mod account {
            pub mod v1 {
                include!("bgs.protocol.account.v1.rs");
            }
        }
        pub mod authentication {
            pub mod v1 {
                include!("bgs.protocol.authentication.v1.rs");
            }
        }
        pub mod challenge {
            pub mod v1 {
                include!("bgs.protocol.challenge.v1.rs");
            }
        }
        pub mod channel {
            pub mod v1 {
                include!("bgs.protocol.channel.v1.rs");
            }
        }
        pub mod config {
            include!("bgs.protocol.config.rs");
        }
        pub mod connection {
            pub mod v1 {
                include!("bgs.protocol.connection.v1.rs");
            }
        }
        pub mod game_utilities {
            pub mod v1 {
                include!("bgs.protocol.game_utilities.v1.rs");
            }
        }
        pub mod games {
            pub mod v1 {
                include!("bgs.protocol.games.v1.rs");
            }
        }
        pub mod notification {
            pub mod v1 {
                include!("bgs.protocol.notification.v1.rs");
            }
        }
        pub mod presence {
            pub mod v1 {
                include!("bgs.protocol.presence.v1.rs");
            }
        }
        pub mod resources {
            pub mod v1 {
                include!("bgs.protocol.resources.v1.rs");
            }
        }
        pub mod session {
            pub mod v1 {
                include!("bgs.protocol.session.v1.rs");
            }
        }
    }
}
