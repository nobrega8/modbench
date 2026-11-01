Certificates for the TLS tests only (see src/tls.rs). Made with openssl:
a CA, a server certificate for localhost and 127.0.0.1 and a client
certificate signed by it, and another CA that signed neither. Valid for 100
years; the keys are public and protect nothing.
