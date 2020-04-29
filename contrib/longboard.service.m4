[Unit]
Requires=postgresql.service
After=postgresql.service

[Service]
Type=simple
ExecStart=BINDIR/longboard

[Install]
WantedBy=multi-user.target

