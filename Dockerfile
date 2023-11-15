FROM golang:alpine

ENV PORT=3000

WORKDIR /app

COPY go.mod ./
#COPY go.sum ./

RUN go mod download

COPY ./ ./

RUN go build -o /stost ./main.go

EXPOSE 3000

CMD [ "/stost" ]