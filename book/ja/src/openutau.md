# OpenUtau で使う

OpenUtau は `Resamplers` ディレクトリでサードパーティのリサンプラーを管理します。
標準のリサンプラープロトコルに従って 13 個の引数が渡され、[ネイティブ UTAU](utau.md) と
まったく同じなので、追加のパラメータテンプレートの設定は不要です。

## 設定手順

[OpenUtau 公式 wiki](https://github.com/openutau/OpenUtau/wiki/Resamplers-and-Wavtools) に従います：

1. `resampler`（Windows は `resampler.exe`）を OpenUtau の `Resamplers` フォルダに入れます：
   * Windows：OpenUtau のプログラムディレクトリ下の `Resamplers`
   * Linux：`~/.local/share/OpenUtau/Resamplers`
   * 実行ファイルを OpenUtau のメインウィンドウにドラッグし、**「Install as resampler」** を選ぶ方法もあります（0.1.119+）
2. レンダラーを **`CLASSIC`** に切り替えます
3. レンダラーの横の **⚙ 歯車アイコン** をクリックし、Resampler ドロップダウンで本リサンプラーを選択します
4. プロジェクトを再レンダリングします

> 任意：`Resamplers` ディレクトリに実行ファイルと同名の `.yaml`（例：`resampler.yaml`）を
> Resampler Manifest として置くと、本エンジンが対応するフラグ（expression）を OpenUtau に
> 宣言でき、expression パネルに推奨値と範囲が表示されます。

## モデルの配置（重要）

リサンプラーをインストールすると、OpenUtau は実行ファイルを自身の
`Resamplers/` ディレクトリへ**コピー**します。レンダリング時の作業ディレクトリは
OpenUtau のインストールルートであり、そこに `models/` は存在しないため、
エンジンは**静かに劣化**します（Stub ボコーダ + 内蔵 DSP F0：エラーなく音は
出ますが、音質は実用になりません）。

モデルディレクトリは以下の順で解決されます：

1. コマンドラインの `--models`（OpenUtau は渡さない。CLI 手動実行専用）
2. 環境変数 `NR_MODELS_DIR`（推奨：一度設定すれば全体で有効）
3. 実行ファイルと**同じ階層**の `models/`（つまり `Resamplers/models/`）
4. 作業ディレクトリの `models/`（リリースパッケージ配置）

OpenUtau では 2 か 3 を推奨します。設定後、`resampler info` を実行し、
ボコーダのバックエンドが `stub` でなくなっている（`onnxruntime` と表示される）
ことを確認してください。

## パスに関する注意

* `Resamplers` ディレクトリに置く（またはドラッグ＆ドロップでインストールする）と、パスは OpenUtau が管理するため手動指定は不要です
* macOS / Linux ではバイナリに実行権限を付与してください：`chmod +x resampler`
* macOS で初回実行時に Gatekeeper にブロックされた場合（バイナリは未署名です）、「システム設定 →
  プライバシーとセキュリティ」で許可するか、次を実行します：`xattr -d com.apple.quarantine ./resampler`
* macOS / Linux で Windows 版リサンプラーを実行するには Wine の設定が必要です（`Tools > Preferences >
  Advanced > Wine Path`）。本リポジトリはネイティブの macOS/Linux 版を提供しているため、Wine は不要です

## 設定が反映されたかの確認

OpenUtau で 1 音レンダリングし、出力された WAV を確認します。同じ引数を
コマンドラインから単独で検証することもできます：

```bash
./resampler render <样本路径> /tmp/check.wav C4 100 "" 0 500 60 -50 100 0 '!120' AA
```

コマンドラインでは鳴るのに OpenUtau では鳴らない場合、問題のほとんどはパスか
権限にあり、エンジン自体ではありません。

## クラシック系リサンプラーとの違い

OpenUtau がデフォルトで同梱する world 系リサンプラー（`worldline` など）は、接合部で
位相合わせを行います。本エンジンはニューラルボコーダの方式で、モデルが合成した波形を
出力します。そのため：

* 音質は自然な発声に近づきますが、**毎回のレンダリング結果は完全に同一**です（ランダム性はありません）
* 長い音符はループ結合に依存します。`He` フラグで強制的に有効化できます
* CPU 推論のため 1 回のレンダリングにはクラシック系リサンプラーより時間がかかり、キャッシュの併用を推奨します

## パフォーマンス

初回レンダリングにはモデル読み込みのオーバーヘッド（数十ミリ秒程度）があります。以降は
1 音あたりの所要時間が主にメルスペクトログラム解析とボコーダの推論で決まります。キャッシュを
有効にすると、同じ入力の再レンダリングが大幅に速くなります。

GPU 加速が必要な場合は、`--features cuda`（NVIDIA）または `--features directml`（Windows）を
付けてご自身でビルドしてください。詳細は[インストール](installation.md)を参照してください。
