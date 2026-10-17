import { useCallback, useState } from "react";

export type Lang = "tr" | "en";

const T: Record<Lang, Record<string, string>> = {
  tr: {
    appTagline: "modern arşiv yöneticisi",
    open: "Arşiv Aç",
    create: "Yeni Arşiv",
    extract: "Çıkar",
    test: "Test Et",
    cancel: "İptal",
    start: "Başlat",
    browse: "Gözat…",
    dropTitle: "Dosyaları buraya bırakın",
    dropHint: "zip, 7z, rar, tar, gz, iso ve daha fazlasını açın",
    archivesFilter: "Arşivler",
    allFiles: "Tüm dosyalar",
    extractTitle: "Arşivi Çıkar",
    destFolder: "Hedef klasör",
    existingFiles: "Var olan dosyalar",
    owOverwrite: "Üzerine yaz",
    owSkip: "Atla",
    owRename: "Yeniden adlandır",
    password: "Parola",
    passwordOptional: "Parola (isteğe bağlı)",
    passwordNeeded: "Arşiv parola korumalı",
    passwordHint: "Devam etmek için parolayı girin",
    createTitle: "Yeni Arşiv Oluştur",
    sources: "Kaynaklar",
    addFiles: "Dosya Ekle",
    addFolder: "Klasör Ekle",
    noSources: "Henüz kaynak seçilmedi",
    output: "Çıktı dosyası",
    format: "Format",
    level: "Sıkıştırma",
    lvlFast: "Hızlı",
    lvlNormal: "Normal",
    lvlMax: "Maksimum",
    split: "Bölme boyutu",
    splitNone: "Yok",
    itemCount: "öğe",
    selectedCount: "seçili",
    clear: "Temizle",
    selectAll: "Tümünü seç",
    searchPlaceholder: "Ara…",
    colName: "Ad",
    colSize: "Boyut",
    colPacked: "Sıkıştırılmış",
    colModified: "Değiştirilmiş",
    ready: "Hazır",
    extracting: "Çıkarılıyor",
    creating: "Oluşturuluyor",
    testing: "Test ediliyor",
    extractDone: "Çıkarma tamamlandı",
    createDone: "Arşiv oluşturuldu",
    testDone: "Arşiv sağlam",
    cancelled: "İptal edildi",
    revealFolder: "Klasörü aç",
    error: "Hata",
    noEntries: "Gösterilecek öğe yok",
    noMatch: "Eşleşen öğe yok",
    truncated: "ilk {n} öğe gösteriliyor",
    totalSize: "toplam",
    items: "öğe",
    theme: "Tema",
    language: "Dil",
    close: "Kapat",
  },
  en: {
    appTagline: "modern archive manager",
    open: "Open Archive",
    create: "New Archive",
    extract: "Extract",
    test: "Test",
    cancel: "Cancel",
    start: "Start",
    browse: "Browse…",
    dropTitle: "Drop files here",
    dropHint: "open zip, 7z, rar, tar, gz, iso and more",
    archivesFilter: "Archives",
    allFiles: "All files",
    extractTitle: "Extract Archive",
    destFolder: "Destination folder",
    existingFiles: "Existing files",
    owOverwrite: "Overwrite",
    owSkip: "Skip",
    owRename: "Rename",
    password: "Password",
    passwordOptional: "Password (optional)",
    passwordNeeded: "This archive is password-protected",
    passwordHint: "Enter the password to continue",
    createTitle: "Create New Archive",
    sources: "Sources",
    addFiles: "Add Files",
    addFolder: "Add Folder",
    noSources: "No sources selected yet",
    output: "Output file",
    format: "Format",
    level: "Compression",
    lvlFast: "Fast",
    lvlNormal: "Normal",
    lvlMax: "Maximum",
    split: "Split size",
    splitNone: "None",
    itemCount: "items",
    selectedCount: "selected",
    clear: "Clear",
    selectAll: "Select all",
    searchPlaceholder: "Search…",
    colName: "Name",
    colSize: "Size",
    colPacked: "Packed",
    colModified: "Modified",
    ready: "Ready",
    extracting: "Extracting",
    creating: "Creating",
    testing: "Testing",
    extractDone: "Extraction completed",
    createDone: "Archive created",
    testDone: "Archive is OK",
    cancelled: "Cancelled",
    revealFolder: "Open folder",
    error: "Error",
    noEntries: "No items to show",
    noMatch: "No matching items",
    truncated: "showing first {n} items",
    totalSize: "total",
    items: "items",
    theme: "Theme",
    language: "Language",
    close: "Close",
  },
};

export function useI18n() {
  const [lang, setLangState] = useState<Lang>(() => {
    const saved = localStorage.getItem("zf.lang");
    if (saved === "tr" || saved === "en") return saved;
    return navigator.language.toLowerCase().startsWith("tr") ? "tr" : "en";
  });

  const setLang = useCallback((l: Lang) => {
    setLangState(l);
    localStorage.setItem("zf.lang", l);
  }, []);

  const t = useCallback(
    (key: string, vars?: Record<string, string | number>) => {
      let s = T[lang][key] ?? key;
      if (vars) {
        for (const [k, v] of Object.entries(vars)) s = s.replace(`{${k}}`, String(v));
      }
      return s;
    },
    [lang],
  );

  return { lang, setLang, t };
}
