//! 邮件下发 — 通过 SMTP 发送邮箱验证码。
//!
//! 未配置 `SMTP_HOST` 时退化为「验证码只写服务端日志」的开发模式，
//! 本地无需真实邮箱即可完成任务，生产环境务必配置 SMTP。

use std::time::Duration;

use lettre::message::{Mailbox, Message};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{SmtpTransport, Transport};

use yukina_shared::config::{Settings, SmtpConfig};
use yukina_shared::error::{ApiError, ApiResult};

/// 发送一封纯文本邮件。
pub async fn send_text(cfg: &Settings, to: &str, subject: &str, body: &str) -> ApiResult<()> {
    let Some(smtp) = cfg.smtp.clone() else {
        tracing::warn!(
            email = %to,
            subject = %subject,
            "未配置 SMTP，邮件内容仅写入日志：\n{body}"
        );
        return Ok(());
    };

    let to = to.to_string();
    let subject = subject.to_string();
    let body = body.to_string();
    match tokio::task::spawn_blocking(move || send_blocking(&smtp, &to, &subject, &body)).await {
        Ok(result) => result,
        Err(err) => Err(ApiError::Internal(format!("邮件任务执行失败: {err}"))),
    }
}

/// 同步发送（lettre 的传输层是阻塞 IO，放在 spawn_blocking 中执行）。
fn send_blocking(smtp: &SmtpConfig, to: &str, subject: &str, body: &str) -> ApiResult<()> {
    let from = Mailbox::new(
        Some(smtp.from_name.clone()),
        smtp.from
            .parse()
            .map_err(|_| ApiError::Internal("SMTP_FROM 发件地址格式不正确".into()))?,
    );
    let to_mailbox = Mailbox::new(
        None,
        to.parse()
            .map_err(|_| ApiError::Internal("收件邮箱格式不正确".into()))?,
    );

    let email = Message::builder()
        .from(from)
        .to(to_mailbox)
        .subject(subject)
        .body(body.to_string())
        .map_err(|err| ApiError::Internal(format!("邮件构造失败: {err}")))?;

    let builder = if smtp.tls {
        SmtpTransport::starttls_relay(&smtp.host)
            .map_err(|err| ApiError::Internal(format!("SMTP 配置无效: {err}")))?
    } else {
        SmtpTransport::builder_dangerous(&smtp.host)
    };
    let mut builder = builder
        .port(smtp.port)
        .timeout(Some(Duration::from_secs(smtp.timeout_seconds)));
    if let (Some(username), Some(password)) = (smtp.username.clone(), smtp.password.clone()) {
        builder = builder.credentials(Credentials::new(username, password));
    }

    builder.build().send(&email).map_err(|err| {
        tracing::error!(email = %to, error = %err, "验证码邮件发送失败");
        ApiError::Internal("验证码邮件发送失败，请稍后再试".into())
    })?;
    Ok(())
}
