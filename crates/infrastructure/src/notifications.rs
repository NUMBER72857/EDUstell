use application::notifications::{EmailMessage, EmailSender, NotificationError};

#[derive(Debug, Default, Clone)]
pub struct NoopEmailSender;

impl EmailSender for NoopEmailSender {
    fn send(&self, _message: EmailMessage) -> Result<(), NotificationError> {
        Ok(())
    }
}
