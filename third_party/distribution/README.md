# ImageLore distribution notices

This directory preserves third-party license and notice texts. Third-party
components retain their own terms; ImageLore's root MIT license covers the
project's original code and does not replace those terms.

`notice-manifest.json` records each text's component, source URL, byte size,
and SHA-256. `rust-source-index.json` gives exact published source archive URLs
and Cargo.lock checksums for all 403 Windows normal/build registry packages.
Source for those components, including the MPL-covered `selectors` crate, is
available from the listed source archive URLs. The source index identifies the
unmodified package versions used by the build.

The collection conservatively includes build dependencies and installed npm
packages. Inclusion is an attribution precaution and does not claim that every
listed component is statically linked into the application. Frontend notices,
native runtime notices, and optional model context are recorded separately.

`THIRD_PARTY_NOTICES.txt` combines the listed third-party texts for convenient
reading; the individual files preserve their original bytes. The Microsoft
Visual C++ Runtime terms are a paragraph-text extraction from Microsoft's
official document; its source URL and complete original-document digest are
recorded in the manifest. Microsoft runtime binaries retain Microsoft terms.

Model weights are downloaded separately and are not included in the installer.
The original CLIP license is provided as model context; this does not assert an
independently reproduced ONNX conversion or a blanket license for other assets.
