# mountix

about *mountix API*

[https://dottrail.codemountains.org/lp/mountix-api/](https://dottrail.codemountains.org/lp/mountix-api/)

## ビルドコマンド

```shell
docker image build -t rust-lambda-build -f Dockerfile.build .
```

```shell
docker container run --rm -v $PWD:/code -v $HOME/.cargo/registry:/root/.cargo/registry -v $HOME/.cargo/git:/root/.cargo/git rust-lambda-build
```

## デプロイコマンド

```shell
sam package --template-file template.yaml --output-template-file packaged.yaml --s3-bucket mountix-api-lambda-function
```

```shell
sam deploy --template-file packaged.yaml --stack-name mountix-api --capabilities CAPABILITY_IAM
```
