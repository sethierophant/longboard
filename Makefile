# Makefile for the longboard project
#
# This file attempts to follow [GNU Makefile conventions][1] where convenient,
# but does not attempt to sacrifice simplicity or maintainability for an
# attempt to be configurable.
#
# [1]: https://www.gnu.org/prep/standards/html_node/Makefile-Conventions.html

SHELL		= /bin/sh

INSTALLFLAGS	= -o longboard -g longboard
INSTALL		= install $(INSTALLFLAGS)
INSTALL_PROGRAM	= $(INSTALL)
INSTALL_DATA	= $(INSTALL) -m 644

CARGOFLAGS	= --locked
CARGO		= cargo $(CARGOFLAGS)

prefix		= /usr/local
exec_prefix	= $(prefix)

bindir		= $(exec_prefix)/bin
datarootdir	= $(prefix)/share
datadir		= $(datarootdir)
mandir		= $(datarootdir)/man
man1dir		= $(mandir)/man1
man5dir		= $(mandir)/man5
man8dir		= $(mandir)/man8
man1ext		= .1
man5ext		= .5
man8ext		= .8
sysconfdir	= $(prefix)/etc
resdir		= $(prefix)/var/lib
logdir		= $(prefix)/var/log

target/release/longboard:
	$(CARGO) build --release

all: target/release/longboard

install: target/release/longboard
	$(INSTALL_PROGRAM) -D target/release/longboard $(DESTDIR)$(bindir)/longboard
	$(INSTALL_PROGRAM) -D target/release/longctl $(DESTDIR)$(bindir)/longctl
	$(INSTALL_DATA) -D contrib/config/release.yaml \
		$(DESTDIR)$(sysconfdir)/longboard/config.yaml
	$(INSTALL_DATA) -D contrib/config/pages/rules.md \
		$(DESTDIR)$(sysconfdir)/longboard/pages/rules.md
	$(INSTALL_DATA) -D res/favicon.png -t $(DESTDIR)$(resdir)/longboard/
	$(INSTALL_DATA) -D res/spoiler.png -t $(DESTDIR)$(resdir)/longboard/
	$(INSTALL_DATA) -D res/banners/* -t $(DESTDIR)$(resdir)/longboard/banners
	$(INSTALL_DATA) -D res/script/* -t $(DESTDIR)$(resdir)/longboard/script
	$(INSTALL_DATA) -D res/style/* -t $(DESTDIR)$(resdir)/longboard/style
	cp -r res/templates $(DESTDIR)$(resdir)/longboard/templates
	chown -R longboard:longboard $(DESTDIR)$(resdir)/longboard/templates
	$(INSTALL_DATA) -d $(DESTDIR)$(resdir)/longboard/uploads
	$(INSTALL_DATA) -d $(DESTDIR)$(logdir)/longboard
	mkdir -p $(DESTDIR)$(man1dir)
	mkdir -p $(DESTDIR)$(man5dir)
	mkdir -p $(DESTDIR)$(man8dir)
	- gzip -c contrib/longctl.1 > $(DESTDIR)$(man1dir)/longctl$(man1ext).gz
	- gzip -c contrib/longboard.5 > $(DESTDIR)$(man5dir)/longboard$(man5ext).gz
	- gzip -c contrib/longboard.8 > $(DESTDIR)$(man8dir)/longboard$(man8ext).gz

uninstall:
	rm -f $(DESTDIR)$(bindir)/longboard
	rm -f $(DESTDIR)$(bindir)/longctl
	rm -rf $(DESTDIR)$(resdir)/longboard
	rm -rf $(DESTDIR)$(logdir)/longboard
	- rm -f $(DESTDIR)$(man5dir)/longboard$(man5ext).gz
	- rm -f $(DESTDIR)$(man8dir)/longboard$(man8ext).gz
	- rm -f $(DESTDIR)$(man1dir)/longctl$(man1ext).gz

clean:
	$(CARGO) clean

check:
	$(CARGO) test
