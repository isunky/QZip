# 7-Zip Sidecar record template

M0 does not bundle 7-Zip. M1 must populate this record before a binary is
committed or distributed:

- exact version and target triple;
- official binary source URL and download checksum;
- license text and source-code location;
- whether QZip modified the binary or source;
- packaging location and build verification result;
- statement that QZip is not an official 7-Zip build.
