## 
OtherPublishのようなコントロールパケットが必要
受信・送信をわけるのが必要
状態の整理とリファクタ、状態の送受信の分離が必要


protocolとして、Towerを採用する
Towerはhttp以外にも使えるらしい
https://github.com/tower-rs/tower/tree/master/guides

メモ：decoderのみのテスト
```
cargo test --package mqtt_decoder --lib mqtt
```


// Request Bodyはhttp=axum用語
towerのhostの実装方法をマスターすべき


## memo
複数のmiddlewareが存在して然るべき
- MQTT着信時
- MQTTパケット確定時
- Publishペイロード処理時

## TODO
送信、受信に使用するbufは単一Connectで送信・受信それぞれで使い回すことができるそう

## TODO2
ビルドできるまでにする(Con-Connack)
exampleを用意する
tests配下にpytestでintegration testを書く