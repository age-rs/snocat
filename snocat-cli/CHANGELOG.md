# Change Log
The format is based on [Keep a Changelog](http://keepachangelog.com/).
This project will adhere to [Semantic Versioning](http://semver.org/),
following the release of version 1.0.0.

<!-- next-header -->

## [Unreleased] - ReleaseDate

## [0.7.0] - 2022-10-29
Revamped networking system to use a `ModularDaemon` for both client and server management.
Protocol is now P2P-capable and uses connection sidedness rather than server or client identity.
Made the authority certificate parameter for client mode optional, as WebPKI is now supported for server certificate verification.

## [0.1.2] - 2021-03-01
Add release and changelog management mechanisms

## [0.1.1] - 2021-03-01
Add crate repository metadata

## [0.1.0] - 2021-02-26
Initial release

<!-- next-url -->
[Unreleased]: https://github.com/Microsoft/snocat/compare/snocat-cli-v0.7.0...HEAD
[0.7.0]: https://github.com/Microsoft/snocat/compare/snocat-cli-v0.1.2...snocat-cli-v0.7.0
[0.1.2]: https://github.com/Microsoft/snocat/compare/v0.1.1...snocat-cli-v0.1.2
[0.1.1]: https://github.com/microsoft/snocat/compare/855fc4beacf4f568a08e848193fba65e6e840fd1...v0.1.1
[0.1.0]: https://github.com/microsoft/snocat/compare/b8d28e83c0bf7010d86eaddcdd212fe72848f6bb...855fc4beacf4f568a08e848193fba65e6e840fd1
