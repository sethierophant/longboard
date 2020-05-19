[Unit]
Requires=postgresql.service
After=postgresql.service

[Service]
Type=exec
ExecStart=BINDIR/longboard
WorkingDirectory=SYSCONFDIR/longboard

[Install]
WantedBy=multi-user.target

