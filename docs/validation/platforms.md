# 平台适配范围

当前实现为 text Webhook 适配，全部等待真实账户与手机收件验收。只允许平台官方 HTTPS 地址，关闭重定向。自动测试使用模拟 HTTP，不是访问真实机器人。

| 平台 | 实现 | 限制 |
| --- | --- | --- |
| 飞书 | 群自定义机器人 text，支持签名 | 每段正文预留后 15000 UTF-8 字节；安全关键词/组织配置需用户检查 |
| 企业微信 | 消息推送/群机器人 text | 每段正文 1800 UTF-8 字节，给序号预留空间；需可用的群机器人地址 |
| 钉钉 | 旧群机器人 text 与签名 | 仅已有可用 Webhook；新用户现行机器人方案未接入 |

总正文最多 64 KiB UTF-8，超过明确标注截断。平台错误码、HTTP 状态及结果不确定状态都分别处理，平台回包内容不原样显示。

核对入口：[飞书自定义机器人](https://open.feishu.cn/document/client-docs/bot-v3/add-custom-bot)、[企业微信群机器人](https://developer.work.weixin.qq.com/document/path/91770)、[钉钉接口](https://open.dingtalk.com/document/orgapp/custom-robot-access)、[钉钉旧机器人提醒](https://open-dingtalk.github.io/developerpedia/docs/learn/bot/webhook/overview/)。部分官方页面需要登录或动态加载，本次工具无法完整读取当前平台条件，因此适配保留“待手机实测”标识，不能承诺新账户开通资格。
