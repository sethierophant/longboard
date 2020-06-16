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
persistdir	= $(localstatedir)/lib
logdir		= $(localstatedir)/log

INSTALLFLAGS	=
INSTALL		= install $(INSTALLFLAGS)
INSTALL_PROGRAM	= $(INSTALL)
INSTALL_DIR     = $(INSTALL) -m 755
INSTALL_PRIVDIR = $(INSTALL) -m 700
INSTALL_DATA	= $(INSTALL) -m 644

CARGOFLAGS	= --locked
CARGOFEATURES	= --all-features
CARGO		= cargo +nightly $(CARGOFLAGS)

M4FLAGS		=
M4DEFINES	= -D BINDIR=$(bindir) \
		  -D SYSCONFDIR=$(sysconfdir) \
		  -D DATADIR=$(datadir) \
		  -D PERSISTDIR=$(persistdir) \
		  -D LOGDIR=$(logdir)
M4		= m4 $(M4DEFINES) $(M4FLAGS)

export bindir
export sysconfdir
export datadir
export persistdir
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
	$(INSTALL_DATA) -D res/favicon.png -t $(DESTDIR)$(datadir)/longboard/
	$(INSTALL_DATA) -D res/spoiler.png -t $(DESTDIR)$(datadir)/longboard/
	$(INSTALL_DATA) -D res/banners/* -t \
		$(DESTDIR)$(datadir)/longboard/banners
	$(INSTALL_DATA) -D res/script/* -t $(DESTDIR)$(datadir)/longboard/script
	$(INSTALL_DATA) -D res/style/* -t $(DESTDIR)$(datadir)/longboard/style
	$(INSTALL_DATA) -D res/fonts/* -t $(DESTDIR)$(datadir)/longboard/fonts
	cp -r res/templates $(DESTDIR)$(datadir)/longboard/templates
	$(INSTALL_DIR) -d $(DESTDIR)$(sysconfdir)/longboard
	$(INSTALL_DIR) -d $(DESTDIR)$(persistdir)/longboard
	$(INSTALL_PRIVDIR) -d $(DESTDIR)$(logdir)/longboard
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
	rm -rf $(DESTDIR)$(datadir)/longboard
	rm -rf $(DESTDIR)$(persistdir)/longboard
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
