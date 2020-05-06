# Makefile for the longboard project
#
# This file attempts to follow [GNU Makefile conventions][1] where convenient,
# but does not attempt to sacrifice simplicity or maintainability for an
# attempt to be configurable.
#
# [1]: https://www.gnu.org/prep/standards/html_node/Makefile-Conventions.html

SHELL		= /bin/sh

prefix		= /usr/local
exec_prefix	= $(prefix)

bindir		= $(exec_prefix)/bin
libdir		= $(exec_prefix)/lib
servicedir	= $(libdir)/systemd/system
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
localstatedir	= $(prefix)/var
resdir		= $(localstatedir)/lib
logdir		= $(localstatedir)/log

INSTALLFLAGS	=
INSTALL		= install $(INSTALLFLAGS)
INSTALL_PROGRAM	= $(INSTALL)
INSTALL_DATA	= $(INSTALL) -m 644

CARGOFLAGS	= --locked
CARGOFEATURES	= --all-features
CARGO		= cargo $(CARGOFLAGS)

M4FLAGS		=
M4DEFINES	= -D BINDIR=$(bindir) -D RESDIR=$(resdir) -D LOGDIR=$(logdir)
M4		= m4 $(M4DEFINES) $(M4FLAGS)

export bindir
export sysconfdir
export resdir
export logdir

.PHONY: target/release/longboard

target/release/longboard:
	$(CARGO) build $(CARGOFEATURES) --release

all: target/release/longboard

install: target/release/longboard
	$(INSTALL_PROGRAM) -D target/release/longboard \
		$(DESTDIR)$(bindir)/longboard
	$(INSTALL_PROGRAM) -D target/release/longctl \
		$(DESTDIR)$(bindir)/longctl
	$(INSTALL_DATA) -D res/favicon.png -t $(DESTDIR)$(resdir)/longboard/
	$(INSTALL_DATA) -D res/spoiler.png -t $(DESTDIR)$(resdir)/longboard/
	$(INSTALL_DATA) -D res/banners/* -t \
		$(DESTDIR)$(resdir)/longboard/banners
	$(INSTALL_DATA) -D res/script/* -t $(DESTDIR)$(resdir)/longboard/script
	$(INSTALL_DATA) -D res/style/* -t $(DESTDIR)$(resdir)/longboard/style
	cp -r res/templates $(DESTDIR)$(resdir)/longboard/templates
	$(INSTALL_DATA) -d $(DESTDIR)$(resdir)/longboard/uploads
	$(INSTALL_DATA) -d $(DESTDIR)$(logdir)/longboard
	mkdir -p $(DESTDIR)$(sysconfdir)/longboard
	$(M4) contrib/config/release.yaml.m4 \
		>$(DESTDIR)$(sysconfdir)/longboard/config.yaml
ifdef servicedir
	mkdir -p $(DESTDIR)$(servicedir)
	$(M4) contrib/longboard.service.m4 \
		>$(DESTDIR)$(servicedir)/longboard.service
endif
ifdef mandir
	mkdir -p $(DESTDIR)$(man1dir)
	mkdir -p $(DESTDIR)$(man5dir)
	mkdir -p $(DESTDIR)$(man8dir)
	gzip -c contrib/longctl.1 > $(DESTDIR)$(man1dir)/longctl$(man1ext).gz
	gzip -c contrib/longboard.5 \
		> $(DESTDIR)$(man5dir)/longboard$(man5ext).gz
	gzip -c contrib/longboard.8 \
		> $(DESTDIR)$(man8dir)/longboard$(man8ext).gz
endif

uninstall:
	rm -f $(DESTDIR)$(bindir)/longboard
	rm -f $(DESTDIR)$(bindir)/longctl
	rm -rf $(DESTDIR)$(resdir)/longboard
	rm -rf $(DESTDIR)$(logdir)/longboard
ifdef servicedir
	rm -f $(DESTDIR)$(servicedir)/longboard.service
endif
ifdef mandir
	rm -f $(DESTDIR)$(man5dir)/longboard$(man5ext).gz
	rm -f $(DESTDIR)$(man8dir)/longboard$(man8ext).gz
	rm -f $(DESTDIR)$(man1dir)/longctl$(man1ext).gz
endif

clean:
	$(CARGO) clean

check:
	$(CARGO) test $(CARGOFEATURES)

# vi:ts=8:sw=8
