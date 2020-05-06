[Unit]
Requires=postgresql.service
After=postgresql.service

[Service]
Type=exec
ExecStart=BINDIR/longboard

[Install]
WantedBy=multi-user.target

