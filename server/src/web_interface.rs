use warp::{Filter, Reply};

#[derive(Serialize, Deserialize)]
struct SearchQuery {
    query: String,
    offset: usize,
}

#[derive(Serialize, Deserialize)]
struct SearchResult {
    total_documents: usize,
    docs: Vec<String>,
}

fn json_body() -> impl Filter<Extract = (SearchQuery,), Error = warp::Rejection> + Clone {
    warp::body::content_length_limit(1024 * 16).and(warp::body::json())
}

async fn search_everything(search_query: SearchQuery, index_interface: IndexInterface) -> std::result::Result<warp::reply::Json, warp::Rejection> {
    let result = index_interface.search_everything(&search_query.query, search_query.offset);
    match result {
        Ok((doc_count, documents)) => Ok(warp::reply::json(&SearchResult {
            total_documents: doc_count,
            docs: documents,
        })),
        Err(e) => {
            tracing::error!("Search error: {}", &e);
            Err(warp::reject())
        }
    }
}

pub fn build_routes(index_interface: IndexInterface) -> impl Filter<Extract = impl Reply, Error = warp::Rejection> + Clone {
    let wrapped_storage = warp::any().map(move || index_interface.clone());
    warp::path("search").and(json_body()).and(wrapped_storage).and(warp::path::end()).and_then(search_everything)
    //.map(|search_query: SearchQuery, index_interface: IndexInterface| search_everything(search_query, index_interface))
}
