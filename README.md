marmita - vendor single-file source dependencies from git
==========================================================
marmita is a minimal binary that vendors a single source file
from a remote git repository at a pinned commit, recording the
result in a plain-text manifest.


Requirements
------------
In order to build marmita you need rustc and libgit2.


Installation
------------
Edit Makefile to match your local setup (marmita is installed
into the /usr/local/bin namespace by default).

Afterwards enter the following command to build and install
marmita:

    make clean install


Running marmita
---------------
Add a dependency at the latest commit:

    marmita add ssh://ijanc@ijanc.org/json/jackson

Pin to a tag or branch:

    marmita add -r v1.3.0 ssh://ijanc@ijanc.org/json/jackson

Refresh every dependency:

    marmita update

Refresh one dependency:

    marmita update http.rs

Remove a dependency:

    marmita rm http.rs

List every recorded dependency:

    marmita list

Print the version:

    marmita -V


Manifest
--------
marmita stores its bookkeeping in vendor/VENDOR.  Each entry is
a header line with the file name followed by tab-indented
attribute lines, separated by blank lines:

    jackson.rs
        origin:	ssh://ijanc@ijanc.org/json/jackson
        ref:	v1.3.0
        commit:	a4e84f6d2f21d8a5f28d17a38ed9968251325714
        date:	2026-04-18

The commit field is the source of truth for what is vendored.
The ref field is optional and only present when add was given
a tag or branch with -r.

The manifest is plain text and may be edited by hand; running
marmita update afterwards is the recommended way to apply the
changes to the working copy.


Download
--------
    got clone ssh://ijanc@ijanc.org/marmita


License
-------
ISC -- see LICENSE.
