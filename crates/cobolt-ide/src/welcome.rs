// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Localised quote pool for the welcome pane. Each language has its own set so
//! the rotating quote follows the IDE's selected language. English carries the
//! full pool; the other languages carry a curated, faithfully-translated subset
//! (scripture references are localised, e.g. `Proverbs` → `Proverbios`).

use crate::i18n::Language;

/// `(author, quote)` pairs.
type Quote = (&'static str, &'static str);

/// The quote pool for `lang`.
pub fn quotes(lang: Language) -> &'static [Quote] {
    match lang {
        Language::Spanish    => ES,
        Language::Portuguese => PT,
        Language::Japanese   => JA,
        Language::Chinese    => ZH,
        Language::English    => EN,
    }
}

const EN: &[Quote] = &[
    ("Alan Kay", "The best way to predict the future is to invent it."),
    ("Bill Gates", "We’re changing the world with technology."),
    ("Steve Jobs", "Technology alone is not enough."),
    ("Steve Jobs", "The best way to create value in the 21st century is to connect Creativity with Technology."),
    ("Steve Jobs", "Everybody in this country should learn how to program a computer, because it teaches you how to think."),
    ("Steve Jobs", "Innovation is the only way to win."),
    ("Steve Jobs", "Every once in a while a revolutionary product comes along that changes everything."),
    ("Linus Torvalds", "Most good programmers do programming not because they expect to get paid or get adulation by the public, but because it is fun to program."),
    ("Bill Gates", "We are not even close to finishing the basic dream of what the PC can be."),
    ("Mark Zuckerberg", "Move fast and break things. Unless you are breaking stuff, you are not moving fast enough."),
    ("Elon Musk", "Any product that needs a manual to work is broken."),
    ("Alan Kay", "Technology is anything invented after you were born."),
    ("Grace Hopper", "A ship in port is safe, but that’s not what ships are built for. Sail out to sea and do new things."),
    ("Grace Hopper", "It’s easier to ask forgiveness than it is to get permission."),
    ("Alan Kay", "Simple things should be simple; complex things should be possible."),
    ("Steve Jobs", "Design is not just what it looks like and feels like. Design is how it works."),
    ("Bill Gates", "Your most unhappy customers are your greatest source of learning."),
    ("Elon Musk", "When something is important enough, you do it even if the odds are not in your favor."),
    ("Alan Kay", "People who are really serious about software should make their own hardware."),
    ("Edsger Dijkstra", "Computer science is no more about computers than astronomy is about telescopes."),
    ("Grace Hopper", "Humans are allergic to change. They love to say, ‘We’ve always done it this way.’"),
    ("Jean Sammet", "I do not consider an assembly language (even a sophisticated one) to be a programming language."),
    ("Sister Mary Kenneth Keller", "We’re having an information explosion, and it’s certainly obvious that information is of no use unless it’s available."),
    ("Tim Berners-Lee", "The Web is more a social creation than a technical one."),
    ("Vint Cerf", "The internet is a reflection of our society and that mirror is going to be reflecting what we see."),
    ("Jeff Bezos", "What we need to do is always lean into the future."),
    ("Satya Nadella", "The true opportunity is to build products that people love and that solve real problems."),
    ("Donald Knuth", "The most important property of a program is whether it accomplishes the intention of its user."),
    ("Linus Torvalds", "If you think your users are idiots, only idiots will use it."),
    ("Elon Musk", "Failure is an option here. If things are not failing, you are not innovating enough."),
    ("Proverbs 1:7", "The fear of the Lord is the beginning of knowledge, but fools despise wisdom and instruction."),
    ("Proverbs 3:5-6", "Trust in the Lord with all your heart and lean not on your own understanding; in all your ways submit to him, and he will make your paths straight."),
    ("Proverbs 2:6", "For the Lord gives wisdom; from his mouth come knowledge and understanding."),
    ("Proverbs 4:7", "The beginning of wisdom is this: Get wisdom. Though it cost all you have, get understanding."),
    ("Proverbs 16:16", "How much better to get wisdom than gold, to get insight rather than silver!"),
    ("Proverbs 20:18", "Plans are established by seeking advice; so if you wage war, obtain guidance."),
    ("Proverbs 21:5", "The plans of the diligent lead to profit as surely as haste leads to poverty."),
    ("Proverbs 16:3", "Commit to the Lord whatever you do, and he will establish your plans."),
    ("Proverbs 12:14", "Wise words bring many benefits, and hard work brings rewards."),
    ("Proverbs 10:14", "The wise store up knowledge, but the mouth of a fool invites ruin."),
    ("Proverbs 16:18", "Pride goes before destruction, a haughty spirit before a fall."),
    ("Proverbs 15:1", "A gentle answer turns away wrath, but a harsh word stirs up anger."),
    ("Proverbs 27:17", "As iron sharpens iron, so one person sharpens another."),
    ("Proverbs 4:23", "Above all else, guard your heart, for everything you do flows from it."),
    ("Proverbs 18:21", "The tongue has the power of life and death, and those who love it will eat its fruit."),
    ("Proverbs 15:14", "A wise person is hungry for knowledge, while the fool feeds on foolishness."),
    ("Proverbs 13:4", "Lazy people want much but get little, but those who work hard will prosper."),
    ("Proverbs 9:10", "The fear of the Lord is the beginning of wisdom, and knowledge of the Holy One is understanding."),
    ("Proverbs 3:15", "She is more precious than rubies; nothing you desire can compare with her."),
    ("Proverbs 3:13", "Blessed are those who find wisdom, those who gain understanding."),
    ("Proverbs 3:17", "Her ways are pleasant ways, and all her paths are peace."),
    ("Proverbs 3:18", "She is a tree of life to those who take hold of her; those who hold her fast will be blessed."),
    ("Proverbs 4:5", "Get wisdom, get understanding; do not forget my words or turn away from them."),
    ("Proverbs 4:18", "The path of the righteous is like the morning sun, shining ever brighter till the full light of day."),
    ("Proverbs 23:12", "Apply your heart to instruction and your ears to words of knowledge."),
    ("Proverbs 3:27", "Do not withhold good from those to whom it is due, when it is in your power to act."),
    ("Proverbs 17:22", "A cheerful heart is good medicine, but a crushed spirit dries up the bones."),
    ("Proverbs 15:13", "A happy heart makes the face cheerful, but heartache crushes the spirit."),
    ("Proverbs 16:24", "Pleasant words are a honeycomb, sweet to the soul and healing to the bones."),
    ("Proverbs 11:8", "The righteous person is rescued from trouble, and it falls on the wicked instead."),
    ("Proverbs 29:11", "A fool gives full vent to his anger, but a wise man keeps himself under control."),
];

const ES: &[Quote] = &[
    ("Alan Kay", "La mejor manera de predecir el futuro es inventarlo."),
    ("Steve Jobs", "La tecnología por sí sola no es suficiente."),
    ("Steve Jobs", "El diseño no es solo cómo se ve y se siente. El diseño es cómo funciona."),
    ("Steve Jobs", "La innovación es la única forma de ganar."),
    ("Bill Gates", "Tus clientes más insatisfechos son tu mayor fuente de aprendizaje."),
    ("Linus Torvalds", "Si crees que tus usuarios son idiotas, solo los idiotas lo usarán."),
    ("Grace Hopper", "Es más fácil pedir perdón que pedir permiso."),
    ("Alan Kay", "Las cosas simples deben ser simples; las complejas, posibles."),
    ("Edsger Dijkstra", "La informática no trata de computadoras más de lo que la astronomía trata de telescopios."),
    ("Elon Musk", "Cuando algo es lo bastante importante, lo haces aunque las probabilidades no estén a tu favor."),
    ("Donald Knuth", "La propiedad más importante de un programa es si cumple la intención de su usuario."),
    ("Proverbios 1:7", "El temor del Señor es el principio del conocimiento; los necios desprecian la sabiduría y la instrucción."),
    ("Proverbios 3:5-6", "Confía en el Señor de todo corazón y no en tu propia inteligencia; reconócelo en todos tus caminos, y él enderezará tus sendas."),
    ("Proverbios 4:7", "La sabiduría es lo principal; adquiere sabiduría, y sobre todas tus posesiones adquiere inteligencia."),
    ("Proverbios 16:18", "Antes del quebrantamiento está la soberbia, y antes de la caída, la altivez de espíritu."),
    ("Proverbios 15:1", "La respuesta amable calma el enojo, pero la palabra hiriente lo enciende."),
    ("Proverbios 27:17", "El hierro se afila con el hierro, y el hombre con su prójimo."),
    ("Proverbios 13:4", "El perezoso desea y nada alcanza, pero el diligente prospera."),
];

const PT: &[Quote] = &[
    ("Alan Kay", "A melhor maneira de prever o futuro é inventá-lo."),
    ("Steve Jobs", "A tecnologia sozinha não basta."),
    ("Steve Jobs", "Design não é só como algo parece; design é como funciona."),
    ("Steve Jobs", "A inovação é a única forma de vencer."),
    ("Bill Gates", "Seus clientes mais insatisfeitos são sua maior fonte de aprendizado."),
    ("Linus Torvalds", "Se você acha que seus usuários são idiotas, só idiotas vão usá-lo."),
    ("Grace Hopper", "É mais fácil pedir perdão do que pedir permissão."),
    ("Alan Kay", "Coisas simples devem ser simples; coisas complexas devem ser possíveis."),
    ("Edsger Dijkstra", "Ciência da computação não trata de computadores mais do que astronomia trata de telescópios."),
    ("Elon Musk", "Quando algo é importante o suficiente, você o faz mesmo que as chances estejam contra você."),
    ("Donald Knuth", "A propriedade mais importante de um programa é se ele cumpre a intenção do usuário."),
    ("Provérbios 1:7", "O temor do Senhor é o princípio do conhecimento; os insensatos desprezam a sabedoria e a instrução."),
    ("Provérbios 3:5-6", "Confie no Senhor de todo o seu coração e não se apoie em seu próprio entendimento; reconheça o Senhor em todos os seus caminhos, e ele endireitará as suas veredas."),
    ("Provérbios 4:7", "O princípio da sabedoria é: adquire a sabedoria; sim, com tudo o que possuis adquire o entendimento."),
    ("Provérbios 16:18", "A soberba precede a ruína, e a altivez do espírito precede a queda."),
    ("Provérbios 15:1", "A resposta branda desvia o furor, mas a palavra dura suscita a ira."),
    ("Provérbios 27:17", "Como o ferro afia o ferro, assim o homem afia o seu companheiro."),
    ("Provérbios 13:4", "O preguiçoso deseja e nada consegue, mas o diligente prospera."),
];

const JA: &[Quote] = &[
    ("Alan Kay", "未来を予測する最善の方法は、それを発明することだ。"),
    ("Steve Jobs", "テクノロジーだけでは十分ではない。"),
    ("Steve Jobs", "デザインとは、見た目や感触だけではない。どう機能するかだ。"),
    ("Steve Jobs", "イノベーションこそが勝つ唯一の道だ。"),
    ("Bill Gates", "最も不満を持つ顧客こそ、最大の学びの源である。"),
    ("Linus Torvalds", "ユーザーを愚かだと思えば、愚かな人しか使わなくなる。"),
    ("Grace Hopper", "許可を求めるより、後で許しを請うほうが簡単だ。"),
    ("Alan Kay", "単純なことは単純に、複雑なことは可能に。"),
    ("Edsger Dijkstra", "計算機科学が計算機についてではないのは、天文学が望遠鏡についてではないのと同じだ。"),
    ("Elon Musk", "本当に重要なことなら、たとえ分が悪くても実行する。"),
    ("Donald Knuth", "プログラムで最も重要なのは、利用者の意図を達成できるかどうかだ。"),
    ("箴言 1:7", "主を畏れることは知識の初め。愚かな者は知恵と諭しを侮る。"),
    ("箴言 3:5-6", "心を尽くして主に信頼し、自分の悟りに頼るな。あなたの道をことごとく主に知らせよ。そうすれば主はあなたの道筋をまっすぐにされる。"),
    ("箴言 4:7", "知恵の初めとして、知恵を得よ。あなたの得たすべてのものを尽くして、悟りを得よ。"),
    ("箴言 16:18", "高ぶりは滅びに先立ち、傲慢な心は倒れに先立つ。"),
    ("箴言 15:1", "柔らかな答えは憤りを静め、激しい言葉は怒りを引き起こす。"),
    ("箴言 27:17", "鉄は鉄をとぐ。人はその友によって磨かれる。"),
    ("箴言 13:4", "怠け者は欲しても得られず、勤勉な者は豊かになる。"),
];

const ZH: &[Quote] = &[
    ("Alan Kay", "预测未来最好的方法就是去创造它。"),
    ("Steve Jobs", "仅有技术是不够的。"),
    ("Steve Jobs", "设计不只是外观和感觉，设计是它如何运作。"),
    ("Steve Jobs", "创新是取胜的唯一途径。"),
    ("Bill Gates", "最不满意的客户，是你最宝贵的学习来源。"),
    ("Linus Torvalds", "如果你把用户当傻瓜，就只有傻瓜会用它。"),
    ("Grace Hopper", "请求原谅比请求许可更容易。"),
    ("Alan Kay", "简单的事应当简单，复杂的事应当可行。"),
    ("Edsger Dijkstra", "计算机科学之于计算机，正如天文学之于望远镜。"),
    ("Elon Musk", "当一件事足够重要时，即使胜算不大，你也要去做。"),
    ("Donald Knuth", "程序最重要的属性，是它是否实现了用户的意图。"),
    ("箴言 1:7", "敬畏耶和华是知识的开端，愚妄人藐视智慧和训诲。"),
    ("箴言 3:5-6", "你要专心仰赖耶和华，不可倚靠自己的聪明；在你一切所行的事上都要认定他，他必指引你的路。"),
    ("箴言 4:7", "智慧为首，所以要得智慧；在你一切所得之内必得聪明。"),
    ("箴言 16:18", "骄傲在败坏以先，狂心在跌倒之前。"),
    ("箴言 15:1", "回答柔和，使怒消退；言语暴戾，触动怒气。"),
    ("箴言 27:17", "铁磨铁，磨出刃来；朋友相感，也是如此。"),
    ("箴言 13:4", "懒惰人羡慕，却无所得；殷勤人必得丰裕。"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Language;

    #[test]
    fn every_language_has_a_nonempty_localized_pool() {
        for lang in [Language::English, Language::Spanish, Language::Portuguese,
                     Language::Japanese, Language::Chinese] {
            let q = quotes(lang);
            assert!(!q.is_empty(), "{lang:?} quote pool is empty");
            // No entry may be blank.
            assert!(q.iter().all(|(a, t)| !a.is_empty() && !t.is_empty()),
                "{lang:?} has a blank quote/author");
        }
        // Non-English pools must actually differ from English (localised).
        assert_ne!(quotes(Language::Spanish)[0].1, quotes(Language::English)[0].1);
    }
}
