# Resource preparation

`Device::prepare_resources` 在当前线程顺序执行准备步骤。通过 `ResourcePreparationContext::step` 发布进度；调用方可把整个任务放到工作线程运行。

步骤内创建的 buffer、texture、sampler、shader、加速结构和 pipeline 自动登记所有权。取消或失败时按创建顺序的逆序释放；缓存命中只归还本次取得的 lease。已有调用方持有的缓存资源继续有效，成功时所有权随返回结果交给调用方。显式 `destroy_*` 会同步移除准备任务的登记。

销毁沿用帧退休队列，等待仍在使用资源的 GPU 工作结束。该逻辑位于共享 Rust frontend，适用于所有后端。
