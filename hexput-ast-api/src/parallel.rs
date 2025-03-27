use std::future::Future;
use tokio::runtime::Runtime;
use tokio::task::JoinSet;


pub fn create_runtime() -> Runtime {
    Runtime::new().expect("Failed to create Tokio runtime")
}


pub async fn process_items<T, U, F, Fut>(
    items: Vec<T>,
    processor: F,
) -> Vec<U>
where
    T: Send + 'static,
    U: Send + 'static,
    F: Fn(T) -> Fut + Send + Sync + Clone + 'static,
    Fut: Future<Output = U> + Send,
{
    
    if items.len() <= 1 {
        let mut results = Vec::with_capacity(items.len());
        for item in items {
            results.push(processor(item).await);
        }
        return results;
    }

    let mut join_set = JoinSet::new();

    
    for item in items {
        let processor_clone = processor.clone();
        join_set.spawn(async move {
            processor_clone(item).await
        });
    }

    
    let mut results = Vec::with_capacity(join_set.len());
    while let Some(result) = join_set.join_next().await {
        if let Ok(value) = result {
            results.push(value);
        }
    }

    results
}



pub fn process_items_sync<T, U, F>(runtime: &Runtime, items: Vec<T>, processor: F) -> Vec<U>
where
    T: Send + 'static,
    U: Send + 'static,
    F: Fn(T, &Runtime) -> U + Send + Sync + Clone + 'static,
{
    
    if items.len() <= 1 {
        return items.into_iter().map(|item| processor(item, runtime)).collect();
    }
    
    
    let mut handles = Vec::with_capacity(items.len());
    
    
    for item in items {
        
        let processor_clone = processor.clone();
        
        
        let handle = runtime.spawn_blocking(move || {
            
            
            (item, processor_clone)
        });
        
        handles.push(handle);
    }
    
    
    let mut results = Vec::with_capacity(handles.len());
    for handle in handles {
        
        if let Ok((item, processor_fn)) = runtime.block_on(handle) {
            
            let result = processor_fn(item, runtime);
            results.push(result);
        }
    }
    
    results
}
