//! Activity batches cut to fit the socket's line limit.

use crate::ipc::{IpcMessage, MAX_MESSAGE_BYTES};

/// Items per request of a batch; the line limit may split it further.
const MAX_BATCH_ITEMS: usize = 200;
/// Room for the request around the items (`IpcRequest` id, message tag).
const REQUEST_OVERHEAD: usize = 1024;

/// The requests that carry `message`: an activity batch is cut so each request stays within the
/// socket's line limit. A batch cut in parts is written part by part, not all or none.
pub fn split(message: IpcMessage) -> Result<Vec<IpcMessage>, String> {
    let IpcMessage::ActivityAdd { items } = message else {
        return Ok(vec![message]);
    };
    let budget = MAX_MESSAGE_BYTES as usize - REQUEST_OVERHEAD;
    let mut requests = Vec::new();
    let mut part = Vec::new();
    let mut bytes = 0;
    for (index, item) in items.into_iter().enumerate() {
        let size = serde_json::to_string(&item)
            .map_err(|e| e.to_string())?
            .len()
            + 1;
        if size > budget {
            return Err(format!("items[{index}] is too large to send"));
        }
        if !part.is_empty() && (bytes + size > budget || part.len() == MAX_BATCH_ITEMS) {
            requests.push(IpcMessage::ActivityAdd {
                items: std::mem::take(&mut part),
            });
            bytes = 0;
        }
        bytes += size;
        part.push(item);
    }
    requests.push(IpcMessage::ActivityAdd { items: part });
    Ok(requests)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::activity::ActivityInput;
    use crate::cli::testing::*;
    use crate::ipc::IpcRequest;

    #[test]
    fn plain_messages_are_not_split() {
        assert_eq!(split(IpcMessage::Status), Ok(vec![IpcMessage::Status]));
        let batch = IpcMessage::ActivityAdd {
            items: vec![input("A")],
        };
        assert_eq!(split(batch.clone()), Ok(vec![batch]));
    }

    fn batch_items(requests: &[IpcMessage]) -> Vec<ActivityInput> {
        requests
            .iter()
            .flat_map(|request| match request {
                IpcMessage::ActivityAdd { items } => items.clone(),
                other => panic!("not a batch: {other:?}"),
            })
            .collect()
    }

    #[test]
    fn a_large_batch_is_cut_into_requests_of_at_most_200_items() {
        let items: Vec<ActivityInput> = (0..450).map(|i| input(&format!("item {i}"))).collect();
        let requests = split(IpcMessage::ActivityAdd {
            items: items.clone(),
        })
        .unwrap();
        let sizes: Vec<usize> = requests
            .iter()
            .map(|r| batch_items(&[r.clone()]).len())
            .collect();
        assert_eq!(sizes, [200, 200, 50]);
        assert_eq!(batch_items(&requests), items);
    }

    #[test]
    fn requests_of_large_items_stay_within_the_line_limit() {
        let items: Vec<ActivityInput> = (0..1000)
            .map(|i| {
                let mut item = input(&format!("item {i}"));
                item.url = Some("u".repeat(2000));
                item
            })
            .collect();
        let requests = split(IpcMessage::ActivityAdd {
            items: items.clone(),
        })
        .unwrap();
        assert!(requests.len() > 1);
        for request in &requests {
            let line = serde_json::to_string(&IpcRequest {
                id: "0123456789abcdef0123456789abcdef0123".into(),
                message: request.clone(),
            })
            .unwrap();
            assert!(line.len() < MAX_MESSAGE_BYTES as usize, "{}", line.len());
        }
        assert_eq!(batch_items(&requests), items);
    }

    #[test]
    fn an_item_too_large_to_send_fails_naming_its_index() {
        let mut huge = input("huge");
        huge.url = Some("u".repeat(MAX_MESSAGE_BYTES as usize));
        let result = split(IpcMessage::ActivityAdd {
            items: vec![input("small"), huge],
        });
        assert_eq!(result, Err("items[1] is too large to send".to_string()));
    }
}
