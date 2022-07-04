use std::time::Duration;

use chromiumoxide::{Browser, BrowserConfig, Element, Page};
use color_eyre::eyre::{self, eyre};
use futures::{future, StreamExt};
use rust_decimal::Decimal;
use tokio::sync::oneshot;

use crate::investment::NewInvestment;

#[tracing::instrument(skip(username, password))]
pub async fn scrape_investment_data(
    username: &str,
    password: &str,
) -> eyre::Result<Vec<NewInvestment>> {
    let config = BrowserConfig::builder().build().map_err(|e| eyre!(e))?;

    tracing::info!("launching browser");

    let (browser, mut handler) = Browser::launch(config).await?;

    tracing::info!("starting handler");

    let (tx, rx) = oneshot::channel();

    let handle = tokio::spawn(async move {
        let handler = async move {
            loop {
                if let Err(e) = handler.next().await.unwrap() {
                    tracing::warn!("{}", e);
                }
            }
        };

        tokio::pin!(handler);
        future::select(handler, rx).await;
    });

    // There may be a race condition in chromiumoxide causing new_page to hang sometimes, this seems to fix it
    tokio::time::sleep(Duration::from_secs(1)).await;

    let page = browser.new_page("about:blank").await?;

    tracing::info!("navigating to login page");

    page.goto("https://secure.vanguardinvestor.co.uk/Login")
        .await?;

    tracing::info!("looking for login form elements");

    let username_input = page
        .find_element("div.form-group.username input[type=\"text\"]")
        .await?;
    let password_input = page
        .find_element("div.form-group.password input[type=\"password\"]")
        .await?;
    let login = page
        .find_element("form.form-login button[type=\"submit\"]")
        .await?;

    tracing::info!("entering login credentials");

    username_input.click().await?.type_str(username).await?;
    password_input.click().await?.type_str(password).await?;

    login
        .call_js_fn("function() { this.click() }", false)
        .await?;

    tracing::info!("waiting for login");

    // Click "Investments" button in side nav
    wait_for_element(
        &page,
        "nav.side-navigation ul.secondary-navigation > li:nth-child(2) a",
    )
    .await?
    .call_js_fn("function(){this.click()}", false)
    .await?;

    tracing::info!("loading investments page");

    // Click "Detailed view" toggle switch
    wait_for_element(&page, "div.toggle-switch label")
        .await?
        .call_js_fn("function(){this.click()}", false)
        .await?;

    tracing::info!("switching to detailed view");

    wait_for_element(&page, "table.table-investments-detailed").await?;
    wait_for_element(&page, "table.table-investments-detailed tr.product-row").await?;

    tracing::info!("finding table rows");

    let rows = page
        .find_elements("table.table-investments-detailed tbody tr.product-row")
        .await?;

    tracing::info!("found {} rows in table", rows.len());

    let mut investments = vec![];

    for row in rows {
        let name = row
            .find_element("td.cell-product-name .content-product-name")
            .await?
            .call_js_fn("function(){return this.innerText}", false)
            .await?
            .result
            .value
            .expect("failed to get cell value");

        let name = name.as_str().unwrap();

        if name == "Cash" {
            continue;
        }

        let values = row
            .find_elements("td.cell-money")
            .await?
            .into_iter()
            .map(parse_cell);

        async fn parse_cell(cell: Element) -> eyre::Result<Decimal> {
            let value = cell
                .call_js_fn("function(){return this.innerText}", false)
                .await?
                .result
                .value;

            let value: &str = match value.as_ref() {
                Some(v) => v.as_str().unwrap(),
                None => return Err(eyre!("failed to get cell value")),
            };

            let value = value.trim().replace(['£', '%', ','], "").replace('−', "-");

            let value: Decimal = value.parse().expect("failed to parse number");

            Ok(value)
        }

        let mut values = futures::future::join_all(values).await.into_iter();

        let data = NewInvestment {
            name: name.to_owned(),
            ongoing_charge: values.next().unwrap()?,
            units: values.next().unwrap()?,
            avg_unit_cost: values.next().unwrap()?,
            last_price: values.next().unwrap()?,
            total_cost: values.next().unwrap()?,
            value: values.next().unwrap()?,
            change: values.next().unwrap()?,
        };

        investments.push(data);
    }

    page.close().await?;

    tx.send(())
        .expect("failed to send shutdown request to handler");

    handle.await?;

    Ok(investments)
}

async fn wait_for_element(page: &Page, selector: &str) -> eyre::Result<Element> {
    let mut attempts = 0;

    let element = loop {
        match page.find_element(selector).await {
            Ok(e) => break e,
            Err(_) => {
                attempts += 1;

                if attempts == 10 {
                    return Err(eyre!("failed to find element on page"));
                }

                tokio::time::sleep(Duration::from_secs(1)).await
            }
        }
    };

    Ok(element)
}
