(() => {
  const button = document.querySelector('.language-toggle');
  const translatable = document.querySelectorAll('[data-en][data-zh]');
  let language = localStorage.getItem('gh-downloader-language') || 'en';

  function applyLanguage(next) {
    language = next;
    document.documentElement.lang = language === 'zh' ? 'zh-CN' : 'en';
    for (const element of translatable) {
      const value = element.dataset[language];
      if (value.includes('\\n')) element.innerHTML = value.replaceAll('\\n', '<br>');
      else element.textContent = value;
    }
    button.textContent = language === 'en' ? '中文' : 'EN';
    button.setAttribute('aria-label', language === 'en' ? '切换到中文' : 'Switch to English');
    localStorage.setItem('gh-downloader-language', language);
  }

  button.addEventListener('click', () => applyLanguage(language === 'en' ? 'zh' : 'en'));
  applyLanguage(language);
})();
