pub(crate) const ORIGIN: &str = "https://iw68lh.aliwork.com";
pub(crate) const COOKIE: &str = "isg=BGpqwESq96zc_ntA73hqkgqyu9YM2-41v5mXHfQhLL1IJw_h3G_pRd0UslM7s2bN; tianshu_corp_user=ding2b4c83bec54a29c6f2c783f7214b6d69_FREEUSER; tianshu_csrf_token=c5683320-e1de-4fc0-b89d-65b268eaacd1; c_csrf=c5683320-e1de-4fc0-b89d-65b268eaacd1; cookie_visitor_id=WfkHnTNp; tianshu_app_type=APP_GRVPTEOQ6D4B7FLZFYNJ; JSESSIONID=872E09D38EDC3118067499E5A0303485";
pub(crate) const CSRF_TOKEN: &str = "c5683320-e1de-4fc0-b89d-65b268eaacd1";
pub(crate) const FORM_UUID: &str = "FORM-2768FF7B2C0D4A0AB692FD28DBA09FD57IHQ";
pub(crate) const APP_TYPE: &str = "APP_GRVPTEOQ6D4B7FLZFYNJ";
pub(crate) const SCHEMA_VERSION: &str = "669";
pub(crate) const ACCEPT: &str = "application/json, text/json";
pub(crate) const ACCEPT_LANGUAGE: &str = "zh-CN,zh;q=0.9,ja-JP;q=0.8,ja;q=0.7";
pub(crate) const BX_V: &str = "2.5.11";
pub(crate) const USER_AGENT: &str =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36";

pub(crate) const AUTH_API_BASE: &str =
    "https://dingtalk.avaryholding.com:8443/dingplus/visitorConnector";
pub(crate) const MOBILE_USER_AGENT: &str =
    "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Mobile/15E148 AliApp(DingTalk/7.6.0)";

pub(crate) const COMPANY: &str = "庆鼎精密电子(淮安)有限公司";
pub(crate) const PART: &str = "淮安第二园区";
pub(crate) const APPLY_TYPE: &str = "一般访客";

pub(crate) fn build_referer(account: &str) -> String {
    format!(
        "https://iw68lh.aliwork.com/o/fk_ybfk?account={}&company=%E5%BA%86%E9%BC%8E%E7%B2%BE%E5%AF%86%E7%94%B5%E5%AD%90(%E6%B7%AE%E5%AE%89)%E6%9C%89%E9%99%90%E5%85%AC%E5%8F%B8&part=%E6%B7%AE%E5%AE%89%E7%AC%AC%E4%BA%8C%E5%9B%AD%E5%8C%BA&applyType=%E4%B8%80%E8%88%AC%E8%AE%BF%E5%AE%A2",
        account
    )
}
