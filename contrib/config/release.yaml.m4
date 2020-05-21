# Configuration file for longboard.
#
# See longboard(5) for more information.

## The site name. This is displayed on the home page.
#site_name: Longboard

## The favicon to be loaded. This file should be a PNG.
#favicon: DATADIR/longboard/favicon.png

## The address that the server binds to.
#address: 0.0.0.0

## The port that the server binds to.
#port: 80

## The file to log to.
#log_file: LOGDIR/longboard.log

## How to connect to the database.
#database_uri: postgres://longboard:@localhost/longboard

## Where the resources (templates, css, javascript, ...) are stored.
#resource_dir: DATADIR/longboard

## Where the user-uploaded files are stored.
#upload_dir: PERSISTDIR/longboard

## Where the staff-added pages are stored.
#pages_dir: /etc/longboard/pages

## List of names to use for anonymous posters.
#names: /etc/longboard/names.txt

## A notice message that will be displayed at the top of each board/thread.
#notice: /etc/longboard/notice.md

## Whether or not to allow users to upload files.
#allow_uploads: true

## The allowed file types for file uploads.
#allow_file_types:
#   - image/png
#   - image/jpeg
#   - image/gif
#   - image/webp
#   - image/pnm
#   - image/tiff
#   - video/webm
#   - video/mp4
#   - video/ogg

## The maximum size for user-uploaded files.
#file_size_limit: 4M

## How long to rate limit posts with the same IP address.
#rate_limit_same_user: 10s

## How long to rate limit posts with identical content.
#rate_limit_same_content: 2m

## Rules to filter posts with. The pattern is parsed as a regex.
## See https://docs.rs/regex/1.3.7/regex/#syntax for regex syntax.
#filter_rules:
#   - pattern: word\s?filter
#     replace_with: language enhancer

## Stylesheets to use. Any style you add here will be selectable by your users
## to use, provided it exists in $RESOURCE_DIR/styles/.
#styles:
#   - blood
#   - peace
#   - nautical
#   - yotsuba

## The list of IP addresses to allow for posting. This list will override both
## block_list and dns_block_list.
#allow_list:
#   - 1.2.3.4

# The list of IP addresses to block from posting.
#block_list:
#   - 4.3.2.1

# The list of DNS block lists to use.
#
# Before uncommenting any of the DNSBLs below, make sure that you meet the
# block list's terms of use. In particular, you cannot use the Spamhaus lists
# for commercial purposes.
#
#dns_block_list:
#
#   Block list provided by EFnet, includes open proxies and Tor exit nodes.
#   - rbl.efnetrbl.org
#
#   Block list provided by Spamhaus, includes IP addresses that "appear to
#   Spamhaus to be under the control of, used by, or made available for use by
#   spammers and abusers in unsolicited bulk email or other types of
#   Internet-based abuse that threatens networks or users".
#   - sbl.spamhaus.org
#
#   Block list provided by Spamhaus, includes IP addresses of "hijacked PCs
#   infected by illegal 3rd party exploits, including open proxies (HTTP,
#   socks, AnalogX, wingate, etc), worms/viruses with built-in spam engines,
#   and other types of trojan-horse exploits".
#   - xbl.spamhaus.org
#
#   Block list provided by Cisco, primarily intended to be used for email spam.
#   - bl.spamcop.net
#
#   Block list provided by Proofpoint, Inc., includes "email servers suspected
#   of sending or relaying spam, servers that have been hacked and hijacked,
#   and those with Trojan infestations".
#  - dnsbl.sorbs.net
