FROM alpine:latest

RUN apk --no-cache add chromium ca-certificates && \
    update-ca-certificates && \
    adduser -D chrome

COPY target/release/vanguard /usr/local/bin/vanguard
RUN chmod +x /usr/local/bin/vanguard

USER chrome
WORKDIR /app

EXPOSE 8000

CMD ["/usr/local/bin/vanguard"]
